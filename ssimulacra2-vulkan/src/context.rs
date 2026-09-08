// Minimal clean-room Vulkan compute context: instance -> device -> one-shot
// compute dispatch with staging upload/readback. Correctness-first; no
// allocator crate (manual vkAllocateMemory), no pipeline caching yet.
//
// Provenance: independently implemented against the ash 0.38 source API and the
// Vulkan 1.x spec. Design lessons (not code) from the sibling dssim-vulkan
// port: serialize instance creation (AMD Windows driver returns VK_INCOMPLETE
// on concurrent creation), prefer discrete GPU, validation in dev builds.
use ash::vk;
use std::ffi::CStr;
use std::sync::Mutex;

/// AMD Windows driver fact (observed by the sibling port on this host):
/// two threads creating instances concurrently can return VK_INCOMPLETE.
static CTX_CREATE_LOCK: Mutex<()> = Mutex::new(());

pub struct GpuBuffer {
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,
    pub size: vk::DeviceSize,
}

pub struct VkContext {
    #[allow(dead_code)]
    entry: ash::Entry,
    instance: ash::Instance,
    #[allow(dead_code)]
    physical: vk::PhysicalDevice,
    device: ash::Device,
    queue: vk::Queue,
    #[allow(dead_code)]
    queue_family: u32,
    cmd_pool: vk::CommandPool,
    mem_props: vk::PhysicalDeviceMemoryProperties,
    device_name: String,
    fma_ieee: bool,
}

fn cstr(s: &'static [u8]) -> &'static CStr {
    CStr::from_bytes_with_nul(s).unwrap()
}

impl VkContext {
    /// Create (or fail with a readable reason). Serialized process-wide.
    pub fn new() -> Result<Self, String> {
        let _guard = CTX_CREATE_LOCK.lock().map_err(|e| e.to_string())?;
        let entry = unsafe { ash::Entry::load() }.map_err(|e| format!("entry load: {e:?}"))?;

        let app = vk::ApplicationInfo::default()
            .application_name(cstr(b"ssimulacra2-vulkan\0"))
            .application_version(1)
            .engine_name(cstr(b"none\0"))
            .engine_version(0)
            .api_version(vk::make_api_version(0, 1, 1, 0));

        let mut layer_names: Vec<&'static CStr> = vec![];
        if cfg!(debug_assertions) {
            let avail = unsafe { entry.enumerate_instance_layer_properties() }
                .map_err(|e| format!("layers: {e:?}"))?;
            let want = cstr(b"VK_LAYER_KHRONOS_validation\0");
            if avail.iter().any(|p| unsafe {
                CStr::from_ptr(p.layer_name.as_ptr()) == want
            }) {
                layer_names.push(want);
            }
        }
        let layer_ptrs: Vec<*const i8> = layer_names.iter().map(|l| l.as_ptr()).collect();

        let ici = vk::InstanceCreateInfo::default()
            .application_info(&app)
            .enabled_layer_names(&layer_ptrs);
        let instance = unsafe { entry.create_instance(&ici, None) }
            .map_err(|e| format!("instance: {e:?}"))?;

        let devices = unsafe { instance.enumerate_physical_devices() }
            .map_err(|e| format!("phys: {e:?}"))?;
        let mut best: Option<(vk::PhysicalDevice, u32, i32, String)> = None;
        for d in devices {
            let props = unsafe { instance.get_physical_device_properties(d) };
            let score = match props.device_type {
                vk::PhysicalDeviceType::DISCRETE_GPU => 3,
                vk::PhysicalDeviceType::INTEGRATED_GPU => 2,
                _ => 1,
            };
            let families =
                unsafe { instance.get_physical_device_queue_family_properties(d) };
            for (i, f) in families.iter().enumerate() {
                if f.queue_flags.contains(vk::QueueFlags::COMPUTE) {
                    if best.as_ref().map(|b| b.2).unwrap_or(0) < score {
                        best = Some((d, i as u32, score, device_name(&props.device_name)));
                    }
                    break;
                }
            }
        }
        let (physical, queue_family, _, device_name) =
            best.ok_or_else(|| "no compute-capable device".to_string())?;

        let prio = [1.0f32];
        let qis = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family)
            .queue_priorities(&prio)];
        // maps_combine uses f64 division (correctly-rounded quotient to match
        // the oracle's f32 fdiv); requires the shaderFloat64 feature enabled.
        let feats = unsafe { instance.get_physical_device_features(physical) };
        if feats.shader_float64 == vk::FALSE {
            return Err("device lacks shaderFloat64".into());
        }
        let enabled = vk::PhysicalDeviceFeatures::default().shader_float64(true);
        let dci = vk::DeviceCreateInfo::default()
            .queue_create_infos(&qis)
            .enabled_features(&enabled);
        let device = unsafe { instance.create_device(physical, &dci, None) }
            .map_err(|e| format!("device: {e:?}"))?;
        let queue = unsafe { device.get_device_queue(queue_family, 0) };
        let mem_props = unsafe { instance.get_physical_device_memory_properties(physical) };

        let cpci = vk::CommandPoolCreateInfo::default()
            .queue_family_index(queue_family)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let cmd_pool = unsafe { device.create_command_pool(&cpci, None) }
            .map_err(|e| format!("cmdpool: {e:?}"))?;

        Ok(Self {
            entry,
            instance,
            physical,
            device,
            queue,
            queue_family,
            cmd_pool,
            mem_props,
            device_name,
            fma_ieee: false,
        }
        .with_probe()
        )
    }

    /// Run the fma fingerprint probe; a device whose fma() is not the
    /// correctly-rounded IEEE fma cannot reproduce the oracle's SIMD MulAdd
    /// bit-for-bit, so ulp-level parity tests must not gate on it.
    fn with_probe(mut self) -> Self {
        self.fma_ieee = self.fma_probe().unwrap_or_default();
        self
    }

    fn fma_probe(&self) -> Result<bool, String> {
        let a: f32 = f32::from_bits(0x3f80_0001);
        let b: f32 = f32::from_bits(0x3f80_0002);
        let c: f32 = -1.0f32;
        let expected = a.mul_add(b, c); // IEEE-correct on CPU
        let buf = self.create_empty(4)?;
        let mut push = Vec::with_capacity(12);
        push.extend_from_slice(&a.to_bits().to_le_bytes());
        push.extend_from_slice(&b.to_bits().to_le_bytes());
        push.extend_from_slice(&c.to_bits().to_le_bytes());
        self.run_compute_push(
            include_bytes!("../shaders/fma_probe.spv"),
            crate::c_main(),
            &[&buf],
            1,
            &push,
        )?;
        let got = self.readback_f32(&buf)?;
        self.destroy_buffer(buf);
        Ok(got[0].to_bits() == expected.to_bits())
    }

    /// True if this device's fma matches IEEE correctly-rounded fma.
    pub fn fma_ieee(&self) -> bool {
        self.fma_ieee
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    fn find_memory_type(&self, filter: u32, need: vk::MemoryPropertyFlags) -> Result<u32, String> {
        for i in 0..self.mem_props.memory_type_count {
            if filter & (1 << i) != 0
                && self.mem_props.memory_types[i as usize].property_flags.contains(need)
            {
                return Ok(i);
            }
        }
        Err("no matching memory type".into())
    }

    unsafe fn alloc_buffer(
        &self,
        size: vk::DeviceSize,
        usage: vk::BufferUsageFlags,
        need: vk::MemoryPropertyFlags,
    ) -> Result<GpuBuffer, String> {
        let bci = vk::BufferCreateInfo::default().size(size).usage(usage);
        let buffer = unsafe { self.device.create_buffer(&bci, None) }
            .map_err(|e| format!("buffer: {e:?}"))?;
        let reqs = unsafe { self.device.get_buffer_memory_requirements(buffer) };
        let type_index = self.find_memory_type(reqs.memory_type_bits, need)?;
        let ami = vk::MemoryAllocateInfo::default()
            .allocation_size(reqs.size)
            .memory_type_index(type_index);
        let memory = unsafe { self.device.allocate_memory(&ami, None) }
            .map_err(|e| format!("alloc: {e:?}"))?;
        unsafe { self.device.bind_buffer_memory(buffer, memory, 0) }
            .map_err(|e| format!("bind: {e:?}"))?;
        Ok(GpuBuffer { buffer, memory, size })
    }

    pub fn create_buffer_f32(&self, data: &[f32]) -> Result<GpuBuffer, String> {
        let bytes = std::mem::size_of_val(data) as vk::DeviceSize;
        unsafe {
            let staging = self.alloc_buffer(
                bytes,
                vk::BufferUsageFlags::TRANSFER_SRC,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            )?;
            let ptr = self
                .device
                .map_memory(staging.memory, 0, bytes, vk::MemoryMapFlags::empty())
                .map_err(|e| format!("map: {e:?}"))?;
            std::ptr::copy_nonoverlapping(
                data.as_ptr() as *const u8,
                ptr as *mut u8,
                bytes as usize,
            );
            self.device.unmap_memory(staging.memory);

            let dev = self.alloc_buffer(
                bytes,
                vk::BufferUsageFlags::TRANSFER_DST
                    | vk::BufferUsageFlags::TRANSFER_SRC
                    | vk::BufferUsageFlags::STORAGE_BUFFER,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )?;
            self.one_shot(|cb| {
                let reg = vk::BufferCopy::default().size(bytes);
                self.device.cmd_copy_buffer(cb, staging.buffer, dev.buffer, &[reg]);
            })?;
            self.device.destroy_buffer(staging.buffer, None);
            self.device.free_memory(staging.memory, None);
            Ok(dev)
        }
    }

    pub fn create_empty(&self, len: usize) -> Result<GpuBuffer, String> {
        unsafe {
            self.alloc_buffer(
                (len * 4) as vk::DeviceSize,
                vk::BufferUsageFlags::TRANSFER_SRC
                    | vk::BufferUsageFlags::TRANSFER_DST
                    | vk::BufferUsageFlags::STORAGE_BUFFER,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )
        }
    }

    pub fn readback_f32(&self, buf: &GpuBuffer) -> Result<Vec<f32>, String> {
        let bytes = buf.size;
        unsafe {
            let staging = self.alloc_buffer(
                bytes,
                vk::BufferUsageFlags::TRANSFER_DST,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            )?;
            self.one_shot(|cb| {
                let reg = vk::BufferCopy::default().size(bytes);
                self.device.cmd_copy_buffer(cb, buf.buffer, staging.buffer, &[reg]);
            })?;
            let ptr = self
                .device
                .map_memory(staging.memory, 0, bytes, vk::MemoryMapFlags::empty())
                .map_err(|e| format!("map: {e:?}"))?;
            let out = std::slice::from_raw_parts(ptr as *const u8, bytes as usize).to_vec();
            self.device.unmap_memory(staging.memory);
            self.device.destroy_buffer(staging.buffer, None);
            self.device.free_memory(staging.memory, None);
            let mut v = vec![0f32; out.len() / 4];
            for (dst, src) in v.iter_mut().zip(out.chunks_exact(4)) {
                *dst = f32::from_le_bytes(src.try_into().unwrap());
            }
            Ok(v)
        }
    }

    /// One-shot submit of a recorded command buffer; waits on its fence.
    /// # Safety
    /// `record` must only use handles valid in this context and record legal commands.
    pub unsafe fn one_shot(&self, record: impl Fn(vk::CommandBuffer)) -> Result<(), String> {
        unsafe {
            let alloc = vk::CommandBufferAllocateInfo::default()
                .command_pool(self.cmd_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1);
            let cb = *self
                .device
                .allocate_command_buffers(&alloc)
                .map_err(|e| format!("cmdbuf: {e:?}"))?
                .first()
                .ok_or("no cmdbuf")?;
            let bi = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
            self.device.begin_command_buffer(cb, &bi).map_err(|e| format!("begin: {e:?}"))?;
            record(cb);
            self.device.end_command_buffer(cb).map_err(|e| format!("end: {e:?}"))?;
            let cbs = [cb];
            let sub = vk::SubmitInfo::default().command_buffers(&cbs);
            let fence = self
                .device
                .create_fence(&vk::FenceCreateInfo::default(), None)
                .map_err(|e| format!("fence: {e:?}"))?;
            self.device
                .queue_submit(self.queue, &[sub], fence)
                .map_err(|e| format!("submit: {e:?}"))?;
            self.device
                .wait_for_fences(&[fence], true, u64::MAX)
                .map_err(|e| format!("wait: {e:?}"))?;
            self.device.destroy_fence(fence, None);
            self.device.free_command_buffers(self.cmd_pool, &[cb]);
        }
        Ok(())
    }

    pub fn device_interface(&self) -> &ash::Device {
        &self.device
    }

    pub fn destroy_buffer(&self, b: GpuBuffer) {
        unsafe {
            self.device.destroy_buffer(b.buffer, None);
            self.device.free_memory(b.memory, None);
        }
    }
}

fn device_name(raw: &[std::os::raw::c_char; 256]) -> String {
    let bytes: Vec<u8> = raw.iter().map(|&c| c as u8).take_while(|&c| c != 0).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

impl Drop for VkContext {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_command_pool(self.cmd_pool, None);
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}
