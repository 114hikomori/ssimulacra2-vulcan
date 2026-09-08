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

#[derive(Clone, Copy)]
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
    max_groups_x: u32,
    validation: bool,
    staging: std::sync::Mutex<Option<GpuBuffer>>,
    pub(crate) pipes: std::sync::Mutex<Pipes>,
    batch: std::sync::Mutex<Option<Batch>>,
}

/// H2.2 (Phase H): one command buffer + one submit + one fence per scale.
/// While a batch is open, run_compute_push records into `cmd` (with explicit
/// compute->compute barriers) instead of submitting per dispatch, and
/// destroy_buffer defers to `trash` (recorded dispatches still reference the
/// buffers until the submit completes). finish() frees the sets, the trash
/// and the command buffer after the fence wait (or without a submit on abort).
struct Batch {
    cmd: vk::CommandBuffer,
    sets: Vec<(vk::DescriptorPool, Vec<vk::DescriptorSet>)>,
    trash: Vec<GpuBuffer>,
}

/// H2.1 (Phase H): per-(shader,bindings,push,entry) reusable Vulkan objects so
/// a dispatch no longer re-parses the SPIR-V, re-creates layout/pipeline (a
/// driver compile) and a descriptor pool every single call - the H0/H1 profile
/// shows ~1 ms of fixed cost per dispatch x ~300 dispatches/image dominates
/// small images. One real VkPipelineCache (not NULL) additionally accumulates
/// driver-compiler products across pipelines AND processes: its data is
/// persisted best-effort next to the OS temp dir (keyed by device name, since
/// cache blobs are not device-portable), speeding cold starts of CI jobs and
/// repeated local runs. All handles are destroyed in VkContext::drop before
/// the device.
pub(crate) struct Pipes {
    pub(crate) objs: std::collections::HashMap<
        (usize, usize, usize, usize),
        crate::pipeline::PipeObjs,
    >,
    pub(crate) cache: vk::PipelineCache,
    cache_file: std::path::PathBuf,
}

fn pipeline_cache_path(device_name: &str) -> std::path::PathBuf {
    if let Ok(p) = std::env::var("S2V_PIPELINE_CACHE") {
        return std::path::PathBuf::from(p);
    }
    let slug: String = device_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    std::env::temp_dir().join(format!("ssimulacra2-vulkan-pipeline-{slug}.cache"))
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
        let mut validation = false;
        if cfg!(debug_assertions) {
            let avail = unsafe { entry.enumerate_instance_layer_properties() }
                .map_err(|e| format!("layers: {e:?}"))?;
            let want = cstr(b"VK_LAYER_KHRONOS_validation\0");
            if avail.iter().any(|p| unsafe {
                CStr::from_ptr(p.layer_name.as_ptr()) == want
            }) {
                layer_names.push(want);
                validation = true;
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

        // F1 (BUG_HUNT): dispatches must respect maxComputeWorkGroupCount; the
        // spec minimum is 65535 in x, which 2048^2-class flat dispatches
        // exceed. Queried here, split into 2D in run_compute_push.
        let max_groups_x = unsafe { instance.get_physical_device_properties(physical) }
            .limits
            .max_compute_work_group_count[0]
            .max(1) as u32;

        // H2.1: real pipeline cache, seeded from disk when the bytes parse
        // (any error just means "cold cache" - spec-required fallback).
        let cache_file = pipeline_cache_path(&device_name);
        let initial = std::fs::read(&cache_file).unwrap_or_default();
        let pcache = unsafe {
            device.create_pipeline_cache(
                &vk::PipelineCacheCreateInfo::default().initial_data(&initial),
                None,
            )
        }
        .or_else(|_| {
            unsafe { device.create_pipeline_cache(&vk::PipelineCacheCreateInfo::default(), None) }
        })
        .map_err(|e| format!("pipeline cache: {e:?}"))?;

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
            max_groups_x,
            validation,
            staging: Default::default(),
            batch: Default::default(),
            pipes: std::sync::Mutex::new(Pipes {
                objs: Default::default(),
                cache: pcache,
                cache_file,
            }),
        }
        .with_probe()
        )
    }

    /// Run the fma fingerprint probe; a device whose fma() is not the
    /// correctly-rounded IEEE fma cannot reproduce the oracle's SIMD MulAdd
    /// bit-for-bit, so ulp-level parity tests must not gate on it.
    fn with_probe(mut self) -> Self {
        self.fma_ieee = self.fma_probe().unwrap_or_default();
        if cfg!(debug_assertions) {
            // F2 (BUG_HUNT): make the validation state visible in CI logs -
            // a silently-absent layer once hid a real VUID violation.
            eprintln!(
                "vulkan: device='{}' validation={} fma_ieee={} max_groups_x={}",
                self.device_name, self.validation, self.fma_ieee, self.max_groups_x
            );
        }
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

    /// Device's maxComputeWorkGroupCount[0] (F1 dispatch splitting).
    pub fn max_groups_x(&self) -> u32 {
        self.max_groups_x
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
            // F8 (BUG_HUNT): staging must be freed on every path - except the
            // one where an open batch takes ownership (copy only recorded, so
            // it must outlive this function; finish_batch frees it post-fence).
            let mut handed_to_batch = false;
            let built = (|| -> Result<GpuBuffer, String> {
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
                if let Some(cmd) = self.batch_cmd() {
                    // H2.2: record the upload copy into the open batch instead
                    // of submitting (the per-dispatch barrier includes a
                    // TRANSFER src stage, so compute reads stay ordered).
                    let reg = vk::BufferCopy::default().size(bytes);
                    self.device.cmd_copy_buffer(cmd, staging.buffer, dev.buffer, &[reg]);
                    self.batch_trash(staging);
                    handed_to_batch = true;
                } else {
                    self.one_shot(|cb| {
                        let reg = vk::BufferCopy::default().size(bytes);
                        self.device.cmd_copy_buffer(cb, staging.buffer, dev.buffer, &[reg]);
                    })?;
                }
                Ok(dev)
            })();
            if !handed_to_batch {
                self.device.destroy_buffer(staging.buffer, None);
                self.device.free_memory(staging.memory, None);
            }
            built
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
        Ok(self.readback_f32_all(&[buf])?.pop().unwrap_or_default())
    }

    /// H1 (Phase H): read several device buffers with ONE staging allocation
    /// (grow-only, cached in self.staging), ONE submit+fence, and ONE
    /// mapped-memory pass. Values are byte-identical to per-buffer
    /// readbacks; only the host-side copying went away (the H0 profile:
    /// fresh 100 MB HOST_COHERENT staging + Vec<u8> copy + zero-init Vec<f32>
    /// + byte loop per call was ~40% of big-image wall time).
    pub fn readback_f32_all(&self, bufs: &[&GpuBuffer]) -> Result<Vec<Vec<f32>>, String> {
        let total: vk::DeviceSize = bufs.iter().map(|b| b.size).sum();
        let mut guard = self
            .staging
            .lock()
            .map_err(|_| "staging lock poisoned".to_string())?;
        let small = guard.as_ref().is_some_and(|s| s.size < total);
        if small || guard.is_none() {
            if let Some(s) = guard.take() {
                unsafe {
                    self.device.destroy_buffer(s.buffer, None);
                    self.device.free_memory(s.memory, None);
                }
            }
            *guard = Some(unsafe {
                // H1 experiment: cached host-visible staging instead of
                // COHERENT (write-combined reads were measuring ~200 MB/s);
                // correctness then REQUIRES invalidate before any CPU read,
                // done below and enforced by the validation layer.
                self.alloc_buffer(
                    total,
                    vk::BufferUsageFlags::TRANSFER_DST,
                    vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_CACHED,
                )
                .or_else(|_| {
                    self.alloc_buffer(
                        total,
                        vk::BufferUsageFlags::TRANSFER_DST,
                        vk::MemoryPropertyFlags::HOST_VISIBLE
                            | vk::MemoryPropertyFlags::HOST_COHERENT,
                    )
                })?
            });
        }
        let staging = guard.as_ref().unwrap();
        let outs = unsafe {
            let sbuf = staging.buffer;
            let copy = |cb: vk::CommandBuffer| {
                let mut off = 0u64;
                for b in bufs {
                    let reg = vk::BufferCopy::default().dst_offset(off).size(b.size);
                    self.device.cmd_copy_buffer(cb, b.buffer, sbuf, &[reg]);
                    off += b.size;
                }
            };
            (|| -> Result<Vec<Vec<f32>>, String> {
                self.one_shot(copy)?;
                // Map the WHOLE cached allocation: a partial mapping whose end
                // is neither nonCoherentAtomSize-aligned (128 on AMD) nor the
                // memory end violates VUID-VkMappedMemoryRange-size-01389/01390
                // when this call's `total` is smaller than the grow-only cache
                // (validation caught it on the odd/s9 fixtures; reading only
                // the first `total` bytes is unaffected).
                let ptr = self
                    .device
                    .map_memory(staging.memory, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())
                    .map_err(|e| format!("map: {e:?}"))? as *const u8;
                // All paths below must unmap before returning (F8-style: the
                // map is a resource). The body computes the result, then we
                // unmap unconditionally and surface any error afterwards.
                let body = (|| -> Result<Vec<Vec<f32>>, String> {
                    // Mandatory for HOST_CACHED (non-coherent) staging before
                    // any CPU read; harmless no-op for the COHERENT fallback.
                    // WholeSize (not the copy length): the cached allocation is
                    // grow-only and may be larger than this readback's bytes,
                    // and per VUID-VkMappedMemoryRange-size-01390 a partial
                    // range must be atom-multiple or exactly the allocation -
                    // non-power-of-two fixtures caught the violation with
                    // stale-cache bit errors within tolerance. Never truncate.
                    let range = vk::MappedMemoryRange::default()
                        .memory(staging.memory)
                        .offset(0)
                        .size(vk::WHOLE_SIZE);
                    self.device
                        .invalidate_mapped_memory_ranges(&[range])
                        .map_err(|e| format!("invalidate: {e:?}"))?;
                    let mut out = Vec::with_capacity(bufs.len());
                    let mut off = 0u64;
                    for b in bufs {
                        let n = (b.size / 4) as usize;
                        let mut v: Vec<f32> = Vec::with_capacity(n);
                        // aligned f32-word copy; staging map is >=64-byte
                        // aligned and every offset is a multiple of 4 bytes.
                        std::ptr::copy_nonoverlapping(
                            ptr.offset(off as isize) as *const f32,
                            v.as_mut_ptr(),
                            n,
                        );
                        v.set_len(n);
                        out.push(v);
                        off += b.size;
                    }
                    Ok(out)
                })();
                self.device.unmap_memory(staging.memory);
                let out = body?;
                Ok(out)
            })()
        }?;
        Ok(outs)
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
            // F8 (BUG_HUNT): fence and command buffer must be released on every path.
            let mut fence = None;
            let r = (|| -> Result<(), String> {
                let bi = vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
                self.device.begin_command_buffer(cb, &bi).map_err(|e| format!("begin: {e:?}"))?;
                record(cb);
                self.device.end_command_buffer(cb).map_err(|e| format!("end: {e:?}"))?;
                let cbs = [cb];
                let sub = vk::SubmitInfo::default().command_buffers(&cbs);
                fence.replace(
                    self.device
                        .create_fence(&vk::FenceCreateInfo::default(), None)
                        .map_err(|e| format!("fence: {e:?}"))?,
                );
                self.device
                    .queue_submit(self.queue, &[sub], fence.unwrap())
                    .map_err(|e| format!("submit: {e:?}"))?;
                self.device
                    .wait_for_fences(&[fence.unwrap()], true, u64::MAX)
                    .map_err(|e| format!("wait: {e:?}"))?;
                Ok(())
            })();
            if let Some(f) = fence {
                self.device.destroy_fence(f, None);
            }
            self.device.free_command_buffers(self.cmd_pool, &[cb]);
            r
        }
    }

    pub fn device_interface(&self) -> &ash::Device {
        &self.device
    }

    pub fn destroy_buffer(&self, b: GpuBuffer) {
        if self.batch_trash(b) {
            return;
        }
        self.destroy_buffer_now(b);
    }

    fn destroy_buffer_now(&self, b: GpuBuffer) {
        unsafe {
            self.device.destroy_buffer(b.buffer, None);
            self.device.free_memory(b.memory, None);
        }
    }

    // ---- H2.2 batch API: one submit + one fence per scale -----------------

    /// Begin recording a batch of dispatches into one command buffer. While
    /// open, run_compute_push records (barrier + dispatch) instead of
    /// submitting, create_buffer_f32 records its upload copy, and
    /// destroy_buffer defers to after the batch fence. Not reentrant; one
    /// batch at a time per context.
    pub fn begin_batch(&self) -> Result<(), String> {
        let alloc = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.cmd_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        let cb = *unsafe { self.device.allocate_command_buffers(&alloc) }
            .map_err(|e| format!("batch cmdbuf: {e:?}"))?
            .first()
            .ok_or("no cmdbuf")?;
        unsafe {
            self.device
                .begin_command_buffer(
                    cb,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .map_err(|e| format!("batch begin: {e:?}"))?;
        }
        let mut g = self.batch.lock().map_err(|_| "batch lock".to_string())?;
        if g.is_some() {
            return Err("nested batch".to_string());
        }
        *g = Some(Batch {
            cmd: cb,
            sets: Vec::new(),
            trash: Vec::new(),
        });
        Ok(())
    }

    /// End recording, submit once, fence-wait once, then release every set,
    /// deferred buffer and the command buffer (all in-use resources outlive
    /// the submit by construction).
    pub fn end_batch(&self) -> Result<(), String> {
        self.finish_batch(true)
    }

    /// Discard an open batch without submitting (error path): recorded commands
    /// never reach the queue, resources are released immediately.
    pub fn abort_batch(&self) -> Result<(), String> {
        self.finish_batch(false)
    }

    fn finish_batch(&self, submit: bool) -> Result<(), String> {
        let st = self
            .batch
            .lock()
            .map_err(|_| "batch lock".to_string())?
            .take()
            .ok_or_else(|| "no batch open".to_string())?;
        let mut r = unsafe { self.device.end_command_buffer(st.cmd) }
            .map_err(|e| format!("batch end: {e:?}"));
        if submit && r.is_ok() {
            unsafe {
                match self.device.create_fence(&vk::FenceCreateInfo::default(), None) {
                    Ok(fence) => {
                        let cbs = [st.cmd];
                        let sub = vk::SubmitInfo::default().command_buffers(&cbs);
                        r = self
                            .device
                            .queue_submit(self.queue, &[sub], fence)
                            .map_err(|e| format!("batch submit: {e:?}"));
                        if r.is_ok() {
                            r = self
                                .device
                                .wait_for_fences(&[fence], true, u64::MAX)
                                .map_err(|e| format!("batch wait: {e:?}"));
                        }
                        self.device.destroy_fence(fence, None);
                    }
                    Err(e) => r = Err(format!("batch fence: {e:?}")),
                }
            }
        }
        unsafe {
            for (pool, sets) in &st.sets {
                let _ = self.device.free_descriptor_sets(*pool, sets);
            }
            for b in &st.trash {
                self.device.destroy_buffer(b.buffer, None);
                self.device.free_memory(b.memory, None);
            }
            self.device.free_command_buffers(self.cmd_pool, &[st.cmd]);
        }
        r
    }

    pub(crate) fn batch_cmd(&self) -> Option<vk::CommandBuffer> {
        self.batch.lock().ok()?.as_ref().map(|s| s.cmd)
    }

    pub(crate) fn batch_track_sets(&self, pool: vk::DescriptorPool, set: vk::DescriptorSet) {
        if let Ok(mut g) = self.batch.lock() {
            if let Some(st) = g.as_mut() {
                // sets grouped by pool for one free call each; linear find is
                // fine at a few dozen pools per batch.
                if let Some(entry) = st.sets.iter_mut().find(|(p, _)| *p == pool) {
                    entry.1.push(set);
                } else {
                    st.sets.push((pool, vec![set]));
                }
            }
        }
    }

    pub(crate) fn batch_trash(&self, b: GpuBuffer) -> bool {
        if let Ok(mut g) = self.batch.lock() {
            if let Some(st) = g.as_mut() {
                st.trash.push(b);
                return true;
            }
        }
        false
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
            // H2.1: every cached object + the pipeline cache must die with the
            // device (validation flags any live object at destroy_device).
            if let Ok(mut p) = self.pipes.lock() {
                for (_, o) in p.objs.drain() {
                    self.device.destroy_descriptor_pool(o.pool, None);
                    self.device.destroy_pipeline(o.pipe, None);
                    self.device.destroy_pipeline_layout(o.pl, None);
                    self.device.destroy_descriptor_set_layout(o.dsl, None);
                    self.device.destroy_shader_module(o.shm, None);
                }
                if let Ok(data) = self.device.get_pipeline_cache_data(p.cache) {
                    let _ = std::fs::write(&p.cache_file, data);
                }
                self.device.destroy_pipeline_cache(p.cache, None);
            }
            if let Ok(mut g) = self.staging.lock() {
                if let Some(s) = g.take() {
                    self.device.destroy_buffer(s.buffer, None);
                    self.device.free_memory(s.memory, None);
                }
            }
            self.device.destroy_command_pool(self.cmd_pool, None);
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}
