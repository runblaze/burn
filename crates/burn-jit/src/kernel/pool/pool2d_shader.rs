use cubecl::{
    cpa,
    ir::{Elem, IntKind, Item, KernelDefinition, Scope, Variable, Visibility},
    InputInfo, KernelExpansion, KernelIntegrator, KernelSettings, OutputInfo,
};
use std::marker::PhantomData;

use crate::{kernel::Kernel, JitElement, JitRuntime};

use super::PoolStrategy;

pub(crate) struct Pool2dComputeShader<P: PoolStrategy, R: JitRuntime, E: JitElement> {
    input: Variable,
    output: Variable,
    indices: Option<Variable>,
    kernel_size: [usize; 2],
    pool_strategy: P,
    _elem: PhantomData<E>,
    _runtime: PhantomData<R>,
}

impl<P: PoolStrategy, R: JitRuntime, E: JitElement> Pool2dComputeShader<P, R, E> {
    fn expand(self, scope: &mut Scope) {
        let input = self.input;
        let output = self.output;
        let id = Variable::AbsolutePos;

        let input_stride_0 = scope.create_local(Elem::UInt);
        let input_stride_1 = scope.create_local(Elem::UInt);
        let input_stride_2 = scope.create_local(Elem::UInt);
        let input_stride_3 = scope.create_local(Elem::UInt);

        let input_shape_0 = scope.create_local(Elem::UInt);
        let input_shape_1 = scope.create_local(Elem::UInt);
        let input_shape_2 = scope.create_local(Elem::UInt);
        let input_shape_3 = scope.create_local(Elem::UInt);

        let output_stride_0 = scope.create_local(Elem::UInt);
        let output_stride_1 = scope.create_local(Elem::UInt);
        let output_stride_2 = scope.create_local(Elem::UInt);
        let output_stride_3 = scope.create_local(Elem::UInt);

        let output_shape_0 = scope.create_local(Elem::UInt);
        let output_shape_1 = scope.create_local(Elem::UInt);
        let output_shape_2 = scope.create_local(Elem::UInt);
        let output_shape_3 = scope.create_local(Elem::UInt);

        cpa!(scope, input_stride_0 = stride(input, 0u32));
        cpa!(scope, input_stride_1 = stride(input, 1u32));
        cpa!(scope, input_stride_2 = stride(input, 2u32));
        cpa!(scope, input_stride_3 = stride(input, 3u32));

        cpa!(scope, input_shape_0 = shape(input, 2u32));
        cpa!(scope, input_shape_1 = shape(input, 3u32));
        cpa!(scope, input_shape_2 = shape(input, 2u32));
        cpa!(scope, input_shape_3 = shape(input, 3u32));

        cpa!(scope, output_stride_0 = stride(output, 0u32));
        cpa!(scope, output_stride_1 = stride(output, 1u32));
        cpa!(scope, output_stride_2 = stride(output, 2u32));
        cpa!(scope, output_stride_3 = stride(output, 3u32));

        cpa!(scope, output_shape_0 = shape(output, 0u32));
        cpa!(scope, output_shape_1 = shape(output, 1u32));
        cpa!(scope, output_shape_2 = shape(output, 2u32));
        cpa!(scope, output_shape_3 = shape(output, 3u32));

        let pool_stride_0 = Variable::GlobalScalar {
            id: 0,
            elem: Elem::UInt,
        };
        let pool_stride_1 = Variable::GlobalScalar {
            id: 1,
            elem: Elem::UInt,
        };
        let dilation_0 = Variable::GlobalScalar {
            id: 2,
            elem: Elem::UInt,
        };
        let dilation_1 = Variable::GlobalScalar {
            id: 3,
            elem: Elem::UInt,
        };
        let padding_0 = Variable::GlobalScalar {
            id: 4,
            elem: Elem::UInt,
        };
        let padding_1 = Variable::GlobalScalar {
            id: 5,
            elem: Elem::UInt,
        };

        let b = scope.create_local(Elem::UInt);
        let c = scope.create_local(Elem::UInt);
        let oh = scope.create_local(Elem::UInt);
        let ow = scope.create_local(Elem::UInt);

        cpa!(scope, b = id / output_stride_0);
        cpa!(scope, b = b % output_shape_0);

        cpa!(scope, c = id / output_stride_1);
        cpa!(scope, c = c % output_shape_1);

        cpa!(scope, oh = id / output_stride_2);
        cpa!(scope, oh = oh % output_shape_2);

        cpa!(scope, ow = id / output_stride_3);
        cpa!(scope, ow = ow % output_shape_3);

        let ih = scope.create_local(Elem::UInt);
        let iw = scope.create_local(Elem::UInt);
        let dilated = scope.create_local(Elem::UInt);

        let ih_pad = scope.create_local(Elem::UInt);
        let iw_pad = scope.create_local(Elem::UInt);
        let result = scope.create_local(input.item());

        let index_input = scope.create_local(Elem::UInt);
        let index_input_0 = scope.create_local(Elem::UInt);
        let index_input_1 = scope.create_local(Elem::UInt);
        let index_input_2 = scope.create_local(Elem::UInt);
        let index_input_3 = scope.create_local(Elem::UInt);
        let idx = scope.create_local(Elem::UInt);

        let within_padding_h = scope.create_local(Elem::Bool);
        let within_padding_w = scope.create_local(Elem::Bool);
        let tmp_padding = scope.create_local(Elem::Bool);
        let border_bottom = scope.create_local(Elem::UInt);
        let border_right = scope.create_local(Elem::UInt);

        cpa!(scope, border_bottom = input_shape_2 + padding_0);
        cpa!(scope, border_right = input_shape_3 + padding_1);

        cpa!(scope, index_input_0 = b * input_stride_0);
        cpa!(scope, index_input_1 = c * input_stride_1);

        let accumulator = self.pool_strategy.initialize(scope, input.item());

        (0..self.kernel_size[0]).for_each(|kh| {
            cpa!(scope, ih = oh * pool_stride_0);
            cpa!(scope, dilated = kh * dilation_0);
            cpa!(scope, ih += dilated);

            cpa!(scope, within_padding_h = ih >= padding_0);
            cpa!(scope, tmp_padding = ih < border_bottom);
            cpa!(scope, within_padding_h = within_padding_h && tmp_padding);

            cpa!(scope, if (within_padding_h).then(|scope| {
                (0..self.kernel_size[1]).for_each(|kw| {
                        cpa!(scope, iw = ow * pool_stride_1);
                        cpa!(scope, dilated = kw * dilation_1);
                        cpa!(scope, iw += dilated);

                        cpa!(scope, within_padding_w = iw >= padding_1);
                        cpa!(scope, tmp_padding = iw < border_right);
                        cpa!(scope, within_padding_w = within_padding_w && tmp_padding);

                        cpa!(scope, if (within_padding_w).then(|scope| {
                            cpa!(scope, ih_pad = ih - padding_0);
                            cpa!(scope, iw_pad = iw - padding_1);

                            cpa!(scope, index_input_2 = ih_pad * input_stride_2);
                            cpa!(scope, idx = index_input_2);
                            cpa!(scope, idx += iw_pad);
                            cpa!(scope, index_input_3 = iw_pad * input_stride_3);

                            cpa!(scope, index_input = index_input_0);
                            cpa!(scope, index_input += index_input_1);
                            cpa!(scope, index_input += index_input_2);
                            cpa!(scope, index_input += index_input_3);

                            cpa!(scope, result = input[index_input]);

                            self.pool_strategy.process_result(scope, accumulator, result, idx);
                        }));
                    });
            }));
        });

        self.pool_strategy
            .assign(scope, id, output, self.indices, accumulator);
    }
}

#[derive(new)]
pub(crate) struct Pool2dEagerKernel<P: PoolStrategy, R: JitRuntime, E: JitElement> {
    kernel_size: [usize; 2],
    pool_strategy: P,
    _runtime: PhantomData<R>,
    _elem: PhantomData<E>,
}

impl<P: PoolStrategy, R: JitRuntime, E: JitElement> Kernel for Pool2dEagerKernel<P, R, E> {
    fn define(&self) -> KernelDefinition {
        let mut scope = Scope::root();
        let item = E::cube_elem().into();

        let input = Variable::GlobalInputArray { id: 0, item };
        let output = Variable::GlobalOutputArray { id: 0, item };
        let indices = if P::with_indices() {
            Some(Variable::GlobalOutputArray {
                id: 1,
                item: Item::new(Elem::Int(IntKind::I32)),
            })
        } else {
            None
        };

        scope.write_global_custom(output);

        Pool2dComputeShader {
            input,
            output,
            indices,
            kernel_size: self.kernel_size,
            pool_strategy: self.pool_strategy.clone(),
            _elem: PhantomData::<E>,
            _runtime: PhantomData::<R>,
        }
        .expand(&mut scope);

        let input = InputInfo::Array {
            item,
            visibility: Visibility::Read,
        };
        let scalars = InputInfo::Scalar {
            elem: Elem::UInt,
            size: 6,
        };
        let output = OutputInfo::Array { item };
        let outputs = if P::with_indices() {
            vec![
                output,
                OutputInfo::Array {
                    item: Item::new(Elem::Int(IntKind::I32)),
                },
            ]
        } else {
            vec![output]
        };

        let info = KernelExpansion {
            inputs: vec![input, scalars],
            outputs,
            scope,
        };

        let settings = KernelSettings::default();
        KernelIntegrator::new(info).integrate(settings)
    }

    fn id(&self) -> cubecl::KernelId {
        cubecl::KernelId::new::<Self>().info((self.kernel_size, self.pool_strategy.clone()))
    }
}
