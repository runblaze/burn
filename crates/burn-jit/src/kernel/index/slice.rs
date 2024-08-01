use crate::{
    element::JitElement, kernel::Kernel, ops::numeric::empty_device, tensor::JitTensor, JitRuntime,
};
use burn_tensor::{ElementConversion, Shape};
use cubecl::{
    cpa,
    frontend::TensorHandleRef,
    ir::{Elem, KernelDefinition, Scope, Variable, Visibility},
    CubeCountSettings, Execution, InputInfo, KernelExpansion, KernelIntegrator, KernelSettings,
    OutputInfo,
};
use std::{marker::PhantomData, ops::Range};

#[derive(new)]
struct SliceEagerKernel<R: JitRuntime, E: JitElement> {
    rank: usize,
    _runtime: PhantomData<R>,
    _elem: PhantomData<E>,
}

pub struct SliceComputeShader {
    input: Variable,
    output: Variable,
    rank: usize,
}

impl SliceComputeShader {
    pub fn expand(self, scope: &mut Scope) {
        let input = self.input;
        let output = self.output;
        let id = Variable::AbsolutePos;

        let offset_input = scope.zero(Elem::UInt);
        let offset_local = scope.create_local(Elem::UInt);

        let stride_input = scope.create_local(Elem::UInt);
        let stride_output = scope.create_local(Elem::UInt);
        let shape_output = scope.create_local(Elem::UInt);
        let range_start = scope.create_local(Elem::UInt);

        for i in 0..self.rank {
            cpa!(scope, stride_input = stride(input, i));
            cpa!(scope, stride_output = stride(output, i));
            cpa!(scope, shape_output = shape(output, i));
            cpa!(
                scope,
                range_start = cast(Variable::GlobalScalar {
                    id: i as u16,
                    elem: Elem::UInt
                })
            );

            cpa!(scope, offset_local = id / stride_output);
            cpa!(scope, offset_local = offset_local % shape_output);
            cpa!(scope, offset_local = offset_local + range_start);
            cpa!(scope, offset_local = offset_local * stride_input);

            cpa!(scope, offset_input += offset_local);
        }

        let result = scope.create_local(input.item());
        cpa!(scope, result = input[offset_input]);
        cpa!(scope, output[id] = result);
    }
}

impl<R: JitRuntime, E: JitElement> Kernel for SliceEagerKernel<R, E> {
    fn define(&self) -> KernelDefinition {
        let mut scope = Scope::root();
        let item = E::cube_elem().into();

        let input = Variable::GlobalInputArray { id: 0, item };
        let output = Variable::GlobalOutputArray { id: 0, item };

        scope.write_global_custom(output);

        SliceComputeShader {
            input,
            output,
            rank: self.rank,
        }
        .expand(&mut scope);

        let input = InputInfo::Array {
            item,
            visibility: Visibility::Read,
        };
        let ranges = InputInfo::Scalar {
            elem: Elem::UInt,
            size: self.rank,
        };
        let output = OutputInfo::Array { item };

        let info = KernelExpansion {
            inputs: vec![input, ranges],
            outputs: vec![output],
            scope,
        };

        let settings = KernelSettings::default();
        KernelIntegrator::new(info).integrate(settings)
    }

    fn id(&self) -> cubecl::KernelId {
        cubecl::KernelId::new::<Self>().info(self.rank)
    }
}

pub(crate) fn slice<R: JitRuntime, E: JitElement, const D1: usize, const D2: usize>(
    tensor: JitTensor<R, E, D1>,
    indices: [Range<usize>; D2],
) -> JitTensor<R, E, D1> {
    let mut dims = tensor.shape.dims;
    for i in 0..D2 {
        dims[i] = indices[i].end - indices[i].start;
    }
    let shape_output = Shape::new(dims);
    let output = empty_device(tensor.client.clone(), tensor.device.clone(), shape_output);
    slice_on_output(tensor, output, indices)
}

pub(crate) fn slice_on_output<R: JitRuntime, E: JitElement, const D1: usize, const D2: usize>(
    tensor: JitTensor<R, E, D1>,
    output: JitTensor<R, E, D1>,
    indices: [Range<usize>; D2],
) -> JitTensor<R, E, D1> {
    let mut scalars: Vec<i32> = Vec::with_capacity(D1);

    for i in 0..D1 {
        let start = indices.get(i).map(|index| index.start).unwrap_or(0);
        scalars.push((start as i32).elem());
    }

    let kernel = SliceEagerKernel::<R, E>::new(D1);

    Execution::start(kernel, tensor.client)
        .inputs(&[TensorHandleRef::<R>::new(
            &tensor.handle,
            &tensor.strides,
            &tensor.shape.dims,
        )])
        .outputs(&[TensorHandleRef::new(
            &output.handle,
            &output.strides,
            &output.shape.dims,
        )])
        .with_scalars(&scalars)
        .execute(CubeCountSettings::Output { pos: 0 });

    output
}
