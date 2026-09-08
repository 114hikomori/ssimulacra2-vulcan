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

impl VkContext {
    /// Dispatch one compute pass over N storage buffers (binding i = buffers[i]).
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
        unsafe {
            let shm = device
                .create_shader_module(
                    &vk::ShaderModuleCreateInfo::default().code(&words),
                    None,
                )
                .map_err(|e| format!("shader module: {e:?}"))?;

            let binds: Vec<vk::DescriptorSetLayoutBinding> = (0..buffers.len())
                .map(|i| {
                    vk::DescriptorSetLayoutBinding::default()
                        .binding(i as u32)
                        .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                        .descriptor_count(1)
                        .stage_flags(vk::ShaderStageFlags::COMPUTE)
                })
                .collect();
            let dsl = device
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default().bindings(&binds),
                    None,
                )
                .map_err(|e| format!("dsl: {e:?}"))?;
            let pc = if push.is_empty() {
                None
            } else {
                Some(vk::PushConstantRange::default()
                    .stage_flags(vk::ShaderStageFlags::COMPUTE)
                    .offset(0)
                    .size(push.len() as u32))
            };
            let pl = device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default()
                        .set_layouts(std::slice::from_ref(&dsl))
                        .push_constant_ranges(pc.as_slice()),
                    None,
                )
                .map_err(|e| format!("pipeline layout: {e:?}"))?;
            let stage = vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::COMPUTE)
                .module(shm)
                .name(entry);
            let cpci = vk::ComputePipelineCreateInfo::default().stage(stage).layout(pl);
            let pipeline = *device
                .create_compute_pipelines(vk::PipelineCache::null(), &[cpci], None)
                .map_err(|e| format!("pipeline: {e:?}"))?
                .first()
                .ok_or("no pipeline")?;

            let pool_sizes = [vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(buffers.len() as u32)];
            let dp = device
                .create_descriptor_pool(
                    &vk::DescriptorPoolCreateInfo::default()
                        .pool_sizes(&pool_sizes)
                        .max_sets(1),
                    None,
                )
                .map_err(|e| format!("dpool: {e:?}"))?;
            let ds = *device
                .allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default()
                        .descriptor_pool(dp)
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
                device.cmd_dispatch(cb, groups_x, 1, 1);
            })?;

            device.destroy_descriptor_pool(dp, None);
            device.destroy_pipeline_layout(pl, None);
            device.destroy_pipeline(pipeline, None);
            device.destroy_descriptor_set_layout(dsl, None);
            device.destroy_shader_module(shm, None);
        }
        Ok(())
    }
}
