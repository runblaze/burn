use crate::{element::JitElement, tensor::JitTensor, JitRuntime};
use cubecl::{prelude::*, KernelId};

use super::SourceTemplate;

/// Kernel source to create a [source](SourceTemplate)
pub trait KernelSource: Send + 'static + Sync {
    /// Convert to [source](SourceTemplate)
    fn source(&self) -> SourceTemplate;
    /// Identifier for the kernel, used for caching kernel compilation.
    fn id(&self) -> KernelId;
}

#[derive(new)]
/// Wraps a [kernel source](KernelSource) into a [cube task](CubeTask).
pub struct SourceKernel<K> {
    kernel_source: K,
    cube_dim: CubeDim,
}

impl<K: KernelSource> CubeTask for SourceKernel<K> {
    fn compile(&self) -> CompiledKernel {
        let source_template = self.kernel_source.source();
        let source = source_template.complete();

        CompiledKernel {
            name: Some(core::any::type_name::<K>()),
            source,
            cube_dim: self.cube_dim,
            shared_mem_bytes: 0,
            debug_info: None,
        }
    }

    fn id(&self) -> cubecl::KernelId {
        self.kernel_source.id()
    }
}

/// Generates kernel source code by replacing some information using templating.
#[macro_export]
macro_rules! kernel_source {
    (
        $struct:ident,
        $file:expr
    ) => {
        /// Generated kernel from a source file.
        #[derive(new)]
        pub struct $struct;

        impl $struct {
            fn source(&self) -> $crate::template::SourceTemplate {
                $crate::template::SourceTemplate::new(include_str!($file))
            }
        }
    };
}

/// Create a vector containing the dimension, strides and shape of tensors.
///
/// # Example
///
/// With two tensors (lhs, rhs)
///
/// | Indexes                  | Value       |
/// |:------------------------:|:-----------:|
/// |           0..1           | D           |
/// |           1..D + 1       | lhs strides |
/// |     (D + 1)..(2 * D + 1) | rhs strides |
/// | (2 * D + 1)..(3 * D + 1) | lhs shape   |
/// | (3 * D + 1)..(4 * D + 1) | rhs shape   |
pub fn build_info<R: JitRuntime, E: JitElement, const D: usize>(
    tensors: &[&JitTensor<R, E, D>],
) -> Vec<u32> {
    let mut info: Vec<u32> = vec![0; tensors.len() * 2 * D + 1];
    info[0] = D as u32;

    let mut current = 1;
    for tensor in tensors.iter() {
        for d in 0..D {
            info[current] = tensor.strides[d] as u32;
            current += 1;
        }
    }
    for tensor in tensors.iter() {
        for d in 0..D {
            info[current] = tensor.shape.dims[d] as u32;
            current += 1;
        }
    }
    info
}
