// Single-shot compute dispatch: builds pipeline + descriptors per call.
// Correctness first; pipeline/descriptor reuse is a Phase H optimization.
use crate::context::{GpuBuffer, VkContext};
use ash::vk;

fn spv_words(bytes: &[u8]) -> Vec<u32> {
    assert!(bytes.len().is_multiple_of(4), "SPIR-V must be 4-byte aligned");
    bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

/// F1 (BUG_HUNT): split a flat group count across x/y so no dimension exceeds
/// the device's maxComputeWorkGroupCount.x. Shaders rebuild the flat index as
/// `y * (NumWorkGroups.x * WorkGroupSize.x) + x`, so gx must be used as the
/// row length by both sides. Pure function - unit-tested for the gy>1 cases
/// that only minimum-spec drivers (llvmpipe: 65535) actually exercise.
pub fn split_groups(groups_x: u32, max_x: u32) -> (u32, u32) {
    let gx = groups_x.min(max_x.max(1)).max(1);
    let gy = groups_x.div_ceil(gx);
    (gx, gy)
}

/// F8 (BUG_HUNT): destroys every handle created so far, on every path.
struct PassRes<'a> {
    device: &'a ash::Device,
    shm: Option<vk::ShaderModule>,
    dsl: Option<vk::DescriptorSetLayout>,
    pl: Option<vk::PipelineLayout>,
    pipe: Option<vk::Pipeline>,
    dp: Option<vk::DescriptorPool>,
}

impl Drop for PassRes<'_> {
    fn drop(&mut self) {
        unsafe {
            if let Some(dp) = self.dp {
                self.device.destroy_descriptor_pool(dp, None);
            }
            if let Some(p) = self.pipe {
                self.device.destroy_pipeline(p, None);
            }
            if let Some(pl) = self.pl {
                self.device.destroy_pipeline_layout(pl, None);
            }
            if let Some(dsl) = self.dsl {
                self.device.destroy_descriptor_set_layout(dsl, None);
            }
            if let Some(shm) = self.shm {
                self.device.destroy_shader_module(shm, None);
            }
        }
    }
}

impl VkContext {
    /// Dispatch one compute pass over N storage buffers (binding i = buffers[i]).
    /// `groups_x` is the number of 64-invocation groups needed for the flat
    /// element count; it is split across x/y so the dispatch stays within
    /// maxComputeWorkGroupCount (F1: shaders read gl_NumWorkGroups.x to
    /// reconstruct the flat index).
    /// Returns Err with a readable reason on any failure.
    pub fn run_compute(
        &self,
        spv: &[u8],
        entry: &std::ffi::CStr,
        buffers: &[&GpuBuffer],
        groups_x: u32,
    ) -> Result<(), String> {
        self.run_compute_push(spv, entry, buffers, groups_x, &[])
    }

    /// As `run_compute`, with raw push-constant bytes (must be 4-byte aligned).
    pub fn run_compute_push(
        &self,
        spv: &[u8],
        entry: &std::ffi::CStr,
        buffers: &[&GpuBuffer],
        groups_x: u32,
        push: &[u8],
    ) -> Result<(), String> {
        let device = self.device_interface();
        let words = spv_words(spv);

        let (gx, gy) = split_groups(groups_x, self.max_groups_x());
        if gy > 65535 {
            return Err(format!(
                "dispatch of {groups_x} groups exceeds device 2D workgroup limit \
                 ({}/{})",
                self.max_groups_x(),
                65535
            ));
        }

        unsafe {
            let mut res = PassRes {
                device,
                shm: None,
                dsl: None,
                pl: None,
                pipe: None,
                dp: None,
            };
            res.shm = Some(
                device
                    .create_shader_module(
                        &vk::ShaderModuleCreateInfo::default().code(&words),
                        None,
                    )
                    .map_err(|e| format!("shader module: {e:?}"))?,
            );

            let binds: Vec<vk::DescriptorSetLayoutBinding> = (0..buffers.len())
                .map(|i| {
                    vk::DescriptorSetLayoutBinding::default()
                        .binding(i as u32)
                        .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                        .descriptor_count(1)
                        .stage_flags(vk::ShaderStageFlags::COMPUTE)
                })
                .collect();
            res.dsl = Some(
                device
                    .create_descriptor_set_layout(
                        &vk::DescriptorSetLayoutCreateInfo::default().bindings(&binds),
                        None,
                    )
                    .map_err(|e| format!("dsl: {e:?}"))?,
            );
            let dsl = res.dsl.unwrap();
            let pc = if push.is_empty() {
                None
            } else {
                Some(vk::PushConstantRange::default()
                    .stage_flags(vk::ShaderStageFlags::COMPUTE)
                    .offset(0)
                    .size(push.len() as u32))
            };
            res.pl = Some(
                device
                    .create_pipeline_layout(
                        &vk::PipelineLayoutCreateInfo::default()
                            .set_layouts(std::slice::from_ref(&dsl))
                            .push_constant_ranges(pc.as_slice()),
                        None,
                    )
                    .map_err(|e| format!("pipeline layout: {e:?}"))?,
            );
            let pl = res.pl.unwrap();
            let stage = vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::COMPUTE)
                .module(res.shm.unwrap())
                .name(entry);
            let cpci = vk::ComputePipelineCreateInfo::default().stage(stage).layout(pl);
            res.pipe = Some(
                *device
                    .create_compute_pipelines(vk::PipelineCache::null(), &[cpci], None)
                    .map_err(|e| format!("pipeline: {e:?}"))?
                    .first()
                    .ok_or("no pipeline")?,
            );
            let pipeline = res.pipe.unwrap();

            let pool_sizes = [vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(buffers.len() as u32)];
            res.dp = Some(
                device
                    .create_descriptor_pool(
                        &vk::DescriptorPoolCreateInfo::default()
                            .pool_sizes(&pool_sizes)
                            .max_sets(1),
                        None,
                    )
                    .map_err(|e| format!("dpool: {e:?}"))?,
            );
            let ds = *device
                .allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default()
                        .descriptor_pool(res.dp.unwrap())
                        .set_layouts(std::slice::from_ref(&dsl)),
                )
                .map_err(|e| format!("dsets: {e:?}"))?
                .first()
                .ok_or("no dset")?;

            let infos: Vec<vk::DescriptorBufferInfo> = buffers
                .iter()
                .map(|b| {
                    vk::DescriptorBufferInfo::default()
                        .buffer(b.buffer)
                        .offset(0)
                        .range(b.size)
                })
                .collect();
            let writes: Vec<vk::WriteDescriptorSet> = infos
                .iter()
                .enumerate()
                .map(|(i, info)| {
                    vk::WriteDescriptorSet::default()
                        .dst_set(ds)
                        .dst_binding(i as u32)
                        .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                        .buffer_info(std::slice::from_ref(info))
                })
                .collect();
            device.update_descriptor_sets(&writes, &[]);

            self.one_shot(|cb| {
                device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::COMPUTE, pipeline);
                if !push.is_empty() {
                    device.cmd_push_constants(
                        cb,
                        pl,
                        vk::ShaderStageFlags::COMPUTE,
                        0,
                        push,
                    );
                }
                device.cmd_bind_descriptor_sets(
                    cb,
                    vk::PipelineBindPoint::COMPUTE,
                    pl,
                    0,
                    std::slice::from_ref(&ds),
                    &[],
                );
                device.cmd_dispatch(cb, gx, gy, 1);
            })?;
        }
        Ok(())
    }
}
