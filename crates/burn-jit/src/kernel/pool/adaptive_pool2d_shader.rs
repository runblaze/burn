use cubecl::{
    cpa,
    ir::{Elem, KernelDefinition, Scope, Variable, Visibility},
    InputInfo, KernelExpansion, KernelIntegrator, KernelSettings, OutputInfo,
};
use std::marker::PhantomData;

use crate::{kernel::Kernel, JitElement, JitRuntime};

pub(crate) struct AdaptivePool2dComputeShader<R: JitRuntime, E: JitElement> {
    input: Variable,
    output: Variable,
    _elem: PhantomData<E>,
    _runtime: PhantomData<R>,
}

impl<R: JitRuntime, E: JitElement> AdaptivePool2dComputeShader<R, E> {
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

        let ih_start = Self::start_index(scope, oh, output_shape_2, input_shape_2);
        let ih_end = Self::end_index(scope, oh, output_shape_2, input_shape_2);
        let iw_start = Self::start_index(scope, ow, output_shape_3, input_shape_3);
        let iw_end = Self::end_index(scope, ow, output_shape_3, input_shape_3);

        let result = scope.create_local(input.item());

        let index_input = scope.create_local(Elem::UInt);
        let index_input_0 = scope.create_local(Elem::UInt);
        let index_input_1 = scope.create_local(Elem::UInt);
        let index_input_2 = scope.create_local(Elem::UInt);
        let index_input_3 = scope.create_local(Elem::UInt);

        cpa!(scope, index_input_0 = b * input_stride_0);
        cpa!(scope, index_input_1 = c * input_stride_1);

        let sum = scope.zero(output.item());

        cpa!(
            scope,
            range(ih_start, ih_end).for_each(|ih, scope| {
                cpa!(
                    scope,
                    range(iw_start, iw_end).for_each(|iw, scope| {
                        cpa!(scope, index_input_2 = ih * input_stride_2);
                        cpa!(scope, index_input_3 = iw * input_stride_3);

                        cpa!(scope, index_input = index_input_0);
                        cpa!(scope, index_input += index_input_1);
                        cpa!(scope, index_input += index_input_2);
                        cpa!(scope, index_input += index_input_3);

                        cpa!(scope, result = input[index_input]);

                        cpa!(scope, sum += result);
                    })
                );
            })
        );

        let count = scope.create_local(Elem::UInt);
        let count_tmp = scope.create_local(Elem::UInt);
        let count_float = scope.create_local(output.item());
        let avg = scope.create_local(output.item());

        cpa!(scope, count = ih_end - ih_start);
        cpa!(scope, count_tmp = iw_end - iw_start);
        cpa!(scope, count *= count_tmp);

        cpa!(scope, count_float = cast(count));
        cpa!(scope, avg = sum / count_float);
        cpa!(scope, output[id] = avg);
    }

    fn start_index(
        scope: &mut Scope,
        output_size_index: Variable,
        output_size: Variable,
        input_size: Variable,
    ) -> Variable {
        let elem = E::cube_elem();
        let numerator_float = scope.create_local(elem);
        let div = scope.create_local(elem);
        let index = scope.create_local(Elem::UInt);

        cpa!(scope, index = output_size_index * input_size);
        cpa!(scope, numerator_float = cast(index));
        cpa!(scope, div = cast(output_size));
        cpa!(scope, div = numerator_float / div);
        cpa!(scope, div = floor(div));
        cpa!(scope, index = cast(div));
        index
    }

    fn end_index(
        scope: &mut Scope,
        output_size_index: Variable,
        output_size: Variable,
        input_size: Variable,
    ) -> Variable {
        let elem = E::cube_elem();
        let numerator_float = scope.create_local(elem);
        let div = scope.create_local(elem);
        let index = scope.create_local(Elem::UInt);
        let min = scope.create_local(Elem::Bool);
        let end_index = scope.create_local(Elem::UInt);

        cpa!(scope, index = output_size_index + 1u32);
        cpa!(scope, index *= input_size);
        cpa!(scope, numerator_float = cast(index));
        cpa!(scope, div = cast(output_size));
        cpa!(scope, div = numerator_float / div);
        cpa!(scope, div = ceil(div));
        cpa!(scope, index = cast(div));

        cpa!(scope, min = input_size < index);
        cpa!(scope, if(min).then(|scope|{
            cpa!(scope, end_index = input_size);
        }).else(|scope|{
            cpa!(scope, end_index = index);
        }));
        end_index
    }
}

#[derive(new)]
pub(crate) struct AdaptivePool2dEagerKernel<R: JitRuntime, E: JitElement> {
    _runtime: PhantomData<R>,
    _elem: PhantomData<E>,
}

impl<R: JitRuntime, E: JitElement> Kernel for AdaptivePool2dEagerKernel<R, E> {
    fn define(&self) -> KernelDefinition {
        let mut scope = Scope::root();
        let item = E::cube_elem().into();

        let input = Variable::GlobalInputArray { id: 0, item };
        let output = Variable::GlobalOutputArray { id: 0, item };

        scope.write_global_custom(output);

        AdaptivePool2dComputeShader {
            input,
            output,
            _elem: PhantomData::<E>,
            _runtime: PhantomData::<R>,
        }
        .expand(&mut scope);

        let input = InputInfo::Array {
            item,
            visibility: Visibility::Read,
        };

        let output = OutputInfo::Array { item };

        let info = KernelExpansion {
            inputs: vec![input],
            outputs: vec![output],
            scope,
        };

        let settings = KernelSettings::default();
        KernelIntegrator::new(info).integrate(settings)
    }

    fn id(&self) -> cubecl::KernelId {
        cubecl::KernelId::new::<Self>()
    }
}
