use std::sync::Arc;
use crate::{operators, Error};
use crate::operators::{Cast, Constant, CumSum, Div, Expand, GroupNormalization, LayerNormalization, Mul, RMSNormalization, Reshape, Sigmoid, Slice, Squeeze, Transpose, Unsqueeze};
use crate::tensor::{DType, Dimension, Shape, Tensor, TensorData, TensorDataValue};
use crate::weights::WeightManager;

pub fn linear(weight_manager: &impl WeightManager, input: Arc<dyn Tensor>) -> Result<Arc<dyn Tensor>, Error> {
    let bias = weight_manager.get_tensor("bias").ok();
    let extra_axis_idx = input.rank();
    let input = unsqueeze(input, extra_axis_idx as i64)?;
    let mat_out = operators::MatMul::new(
        weight_manager.get_prefix().map(|x| x.to_string()),
        weight_manager.get_tensor("weight")?,
        input
    )?;
    let mat_out = squeeze(mat_out, extra_axis_idx as i64)?;
    if let Some(bias) = bias {
        Ok(operators::Add::new(Some(format!("{}.bias", weight_manager.get_prefix().unwrap())), mat_out, bias)?)
    } else {
        Ok(mat_out)
    }
}

pub fn reshape(input: Arc<dyn Tensor>, dims: Vec<i64>) -> Result<Arc<Reshape>, Error> {
    let shape = Shape::from(&[dims.len()][..]);
    let c = Constant::new(None, TensorData::new(dims.into(), shape)?);
    Ok(Reshape::new(None, input, c)?)
}

pub fn unsqueeze(input: Arc<dyn Tensor>, dim: i64) -> Result<Arc<Unsqueeze>, Error> {
    let c = Constant::new(None, TensorData::new(vec![dim].into(), Shape::from(&[1][..]))?);
    Ok(Unsqueeze::new(None, input, c)?)
}

pub fn squeeze(input: Arc<dyn Tensor>, dim: i64) -> Result<Arc<Squeeze>, Error> {
    let c = Constant::new(None, TensorData::new(vec![dim].into(), Shape::from(&[1][..]))?);
    Ok(Squeeze::new(None, input, c)?)
}

pub fn slice(input: Arc<dyn Tensor>, start: Vec<i64>, end: Vec<i64>) -> Result<Arc<Slice>, Error>  {
    let const_shape = Shape::new(vec![Dimension::new(Some(start.len()), None, None)]);
    let start = Constant::new(None, TensorData::new(start.into(), const_shape.clone())?);
    let end = Constant::new(None, TensorData::new(end.into(), const_shape)?);
    Ok(Slice::new(None, input, start, end, None, None)?)
}

pub fn cast(input: Arc<dyn Tensor>, dtype: DType) -> Arc<dyn Tensor>  {
    if input.dtype() != dtype {
        Cast::new(None, input, dtype)
    } else {
        input
    }
}

pub fn transpose(input: Arc<dyn Tensor>) -> Arc<Transpose> {
    let mut dims: Vec<_> = (0..input.rank() as i64).collect();
    let (a, b) = (dims[input.rank()-2], dims[input.rank()-1]);
    dims[input.rank()-2] = b;
    dims[input.rank()-1] = a;
    Transpose::new(None, input, Some(dims))
}

pub fn layer_norm(weight_manager: &impl WeightManager, input: Arc<dyn Tensor>, epsilon: f32) -> Result<Arc<LayerNormalization>, Error> {
    LayerNormalization::new(
        weight_manager.get_prefix().map(|x| x.to_string()),
        input,
        weight_manager.get_tensor("weight")?,
        weight_manager.get_tensor("bias").ok(),
        -1,
        epsilon,
        1
    )
}

pub fn group_norm(weight_manager: &impl WeightManager, input: Arc<dyn Tensor>, epsilon: f32, num_groups: i64) -> Result<Arc<GroupNormalization>, Error> {
    GroupNormalization::new(
        weight_manager.get_prefix().map(|x| x.to_string()),
        input,
        weight_manager.get_tensor("weight")?,
        weight_manager.get_tensor("bias")?,
        num_groups,
        epsilon
    )
}

pub fn cumsum(input: Arc<dyn Tensor>, axis: i32) -> Result<Arc<CumSum>, Error> {
    let shape = Shape::new(vec![Dimension::new(Some(1), None, None)]);
    let axis = Constant::new(None, TensorData::fill(shape, axis)?);
    CumSum::new(None, input, axis)
}

pub fn rms_norm(weight_manager: &impl WeightManager, input: Arc<dyn Tensor>) -> Result<Arc<RMSNormalization>, Error> {
    RMSNormalization::new(
        weight_manager.get_prefix().map(|x| x.to_string()),
        input,
        weight_manager.get_tensor("weight")?,
        1e-5,
        -1
    )
}

pub fn silu(input: Arc<dyn Tensor>) -> Result<Arc<dyn Tensor>, Error> {
    let x = Sigmoid::new(None, input.clone());
    Ok(Mul::new(None, input.clone(), x)?)
}

pub fn swiglu(weight_manager: &impl WeightManager, input: Arc<dyn Tensor>) -> Result<Arc<dyn Tensor>, Error> {
    let x = linear(&weight_manager.prefix("linear_inner"), input.clone())?;
    let x = silu(x)?;
    let x2 = linear(&weight_manager.prefix("linear_outer"), input.clone())?;
    let out = Mul::new(None, x, x2)?;
    Ok(out)
}

pub fn div_scalar<T>(input: Arc<dyn Tensor>, scalar: T) -> Result<Arc<Div>, Error>
where
    T: Copy,
    TensorDataValue: From<Vec<T>>
{
    let shape = Shape::new(vec![Dimension::new(Some(1), None, None)]);
    let constant = Constant::new(None, TensorData::fill(shape, scalar)?);
    
    Ok(Div::new(None, input, constant)?)
}

pub fn expand(input: Arc<dyn Tensor>, dims: Vec<i64>) -> Result<Arc<Expand>, Error> {
    let shape = Shape::from(&[dims.len()][..]);
    let c = Constant::new(None, TensorData::new(dims.into(), shape)?);
    Ok(Expand::new(None, input, c)?)
}