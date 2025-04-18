use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::default::Default;
use std::sync::Arc;
use crate::tensor::{Shape, TensorData};
use crate::tensor::Tensor;
use crate::DType;

pub trait Node {
    fn get_input_tensors(&self) -> Vec<&dyn Tensor> {
        vec![]
    }

    fn get_nodes<'a>(&'a self, table: &mut HashSet<&'a dyn Node>) where Self: Sized {
        let dyn_self: &dyn Node = self;
        if !table.contains(&dyn_self) {
            self.get_sub_nodes(table);
            table.insert(dyn_self);
        }
    }

    fn get_sub_nodes<'a>(&'a self, table: &mut HashSet<&'a dyn Node>) {
        for input in self.get_input_tensors() {
            input.get_nodes(table);
        }
    }

    fn get_tensors<'a>(&'a self, table: &mut HashSet<&'a dyn Tensor>) {
        for input in self.get_input_tensors() {
            if !table.contains(&input) {
                table.insert(input);
                input.get_sub_tensors(table);
            }
        }
    }

    fn get_output_tensors(&self) -> Vec<&dyn Tensor>;

    fn get_name(&self) -> Option<&str> {
        None
    }


    fn get_onnx_type(&self) -> &str;
    fn get_onnx_domain(&self) -> &str {
        "ai.onnx"
    }

    fn get_onnx_attributes(&self) -> Vec<crate::onnx::AttributeProto>;

    fn to_node_proto(&self, name: Option<String>, tensor_names: &HashMap<&dyn Tensor, String>) -> crate::onnx::NodeProto {
        crate::onnx::NodeProto {
            name: name.unwrap_or_default(),
            input: self.get_input_tensors().iter().map(|tensor| tensor_names[tensor].clone()).collect(),
            output: self.get_output_tensors().iter().map(|tensor| tensor_names[tensor].clone()).collect(),
            op_type: self.get_onnx_type().to_string(),
            domain: self.get_onnx_domain().to_string(),
            attribute: self.get_onnx_attributes(),
            .. Default::default()
        }
    }
}

impl<'a> PartialEq for &'a dyn Node{
    fn eq(&self, other:&Self) -> bool{
        std::ptr::addr_eq(*self, *other)
    }
}

impl<'a> Eq for &'a dyn Node{}

impl<'a> Hash for &'a dyn Node {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let a: *const _ = *self;
        let address: *const u8 = a.cast();
        state.write_usize(address.addr());
    }
}


pub trait MultiOutputNode: Node {
    fn get_output_shape(&self, output_index: usize) -> &Shape;

    fn get_output_dtype(&self, output_index: usize) -> DType;

    fn get_num_outputs(&self) -> usize;
}

pub(crate) struct MultiOutputNodeOutput {
    parent: Arc<dyn MultiOutputNode>,
    output_index: usize,
}

impl MultiOutputNodeOutput {
    pub(crate) fn new(parent: Arc<dyn MultiOutputNode>, output_index: usize) -> Self {
        Self { parent, output_index }
    }
}

impl Tensor for MultiOutputNodeOutput {
    fn dtype(&self) -> DType {
        self.parent.get_output_dtype(self.output_index)
    }

    fn shape(&self) -> &Shape {
        self.parent.get_output_shape(self.output_index)
    }

    fn get_nodes<'a>(&'a self, table: &mut HashSet<&'a dyn Node>) {
        let dyn_node: &dyn Node = self.parent.as_ref();
        if !table.contains(&dyn_node) {
            self.parent.get_sub_nodes(table);
            table.insert(dyn_node);
        }
    }

    fn get_sub_tensors<'a>(&'a self, table: &mut HashSet<&'a dyn Tensor>) {
        self.parent.get_tensors(table)
    }
}


pub(crate) trait SingleOutputNode: Node {
    fn get_output_shape(&self) -> &Shape;

    fn get_output_dtype(&self) -> DType;
    
    fn resolve_output_data(&self) -> Option<TensorData> {
        None
    }
}