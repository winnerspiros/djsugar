//! Spleeter's U-Nets on burn (any backend; meant for the GPU).

use burn::prelude::*;
use burn::tensor::activation::sigmoid;
use burn::tensor::module::{conv2d, conv_transpose2d};
use burn::tensor::ops::{ConvOptions, ConvTransposeOptions, PadMode};
use burn::tensor::TensorData;
use safetensors::SafeTensors;

use super::{tensor, F, STEMS, T};

struct Layer {
    weight: Tensor<4>,
    bias: Tensor<1>,
    /// Batch norm folded to `x * scale + shift`, per channel.
    scale: Tensor<4>,
    shift: Tensor<4>,
}

struct UNet {
    enc: Vec<Layer>,
    dec: Vec<Layer>,
    out_weight: Tensor<4>,
    out_bias: Tensor<1>,
}

fn elu(x: Tensor<4>) -> Tensor<4> {
    x.clone().clamp_min(0.0) + x.clamp_max(0.0).exp() - 1.0
}

impl Layer {
    fn norm(&self, x: Tensor<4>) -> Tensor<4> {
        x * self.scale.clone() + self.shift.clone()
    }

    /// 5×5 convolution, stride 2. TensorFlow's "same" padding on an even size
    /// pads 1 before and 2 after.
    fn down(&self, x: Tensor<4>) -> Tensor<4> {
        let x = x.pad((1, 2, 1, 2), PadMode::Constant(0.0));
        conv2d(x, self.weight.clone(), Some(self.bias.clone()), ConvOptions::new([2, 2], [0, 0], [1, 1], 1))
    }

    /// 5×5 transposed convolution, stride 2, doubling the size: the full
    /// output is 2n + 3, of which "same" keeps 2n starting at 1.
    fn up(&self, x: Tensor<4>) -> Tensor<4> {
        let [b, _, h, w] = x.dims();
        let y = conv_transpose2d(
            x,
            self.weight.clone(),
            Some(self.bias.clone()),
            ConvTransposeOptions::new([2, 2], [0, 0], [0, 0], [1, 1], 1),
        );
        let c = y.dims()[1];
        y.slice([0..b, 0..c, 1..1 + 2 * h, 1..1 + 2 * w])
    }
}

impl UNet {
    /// The stem's estimated magnitudes for `x`, [batch, 2, T, F].
    fn forward(&self, x: Tensor<4>) -> Tensor<4> {
        let mut convs = Vec::with_capacity(6);
        let mut h = x.clone();
        for layer in &self.enc {
            let c = layer.down(h);
            h = elu(layer.norm(c.clone()));
            convs.push(c);
        }
        // Spleeter's decoder starts from the last convolution before its
        // batch norm and activation, and skips connect pre-norm outputs.
        let mut h = convs.pop().expect("six encoder layers");
        for layer in &self.dec {
            let u = layer.norm(elu(layer.up(h)));
            h = match convs.pop() {
                Some(skip) => Tensor::cat(vec![skip, u], 1),
                None => u,
            };
        }
        let mask = sigmoid(conv2d(
            h,
            self.out_weight.clone(),
            Some(self.out_bias.clone()),
            ConvOptions::new([1, 1], [3, 3], [2, 2], 1),
        ));
        mask * x
    }
}

pub struct Net {
    nets: Vec<UNet>,
    device: Device,
}

impl Net {
    /// Loads the weights file made by `scripts/models/convert_spleeter.py`.
    pub fn load(bytes: &[u8], device: &Device) -> Result<Self, String> {
        let st = SafeTensors::deserialize(bytes).map_err(|e| e.to_string())?;
        let t4 = |name: &str| -> Result<Tensor<4>, String> {
            let (d, s) = tensor(&st, name)?;
            Ok(Tensor::from_data(TensorData::new(d, [s[0], s[1], s[2], s[3]]), device))
        };
        let t1 = |name: &str| -> Result<Tensor<1>, String> {
            let (d, s) = tensor(&st, name)?;
            Ok(Tensor::from_data(TensorData::new(d, [s[0]]), device))
        };
        let channel = |name: &str| -> Result<Tensor<4>, String> {
            let (d, s) = tensor(&st, name)?;
            Ok(Tensor::from_data(TensorData::new(d, [1, s[0], 1, 1]), device))
        };
        let layer = |p: &str| -> Result<Layer, String> {
            Ok(Layer {
                weight: t4(&format!("{p}.weight"))?,
                bias: t1(&format!("{p}.bias"))?,
                scale: channel(&format!("{p}.bn_scale"))?,
                shift: channel(&format!("{p}.bn_shift"))?,
            })
        };
        let nets = STEMS
            .iter()
            .map(|stem| {
                Ok(UNet {
                    enc: (0..6).map(|k| layer(&format!("{stem}.enc{k}"))).collect::<Result<_, String>>()?,
                    dec: (0..6).map(|k| layer(&format!("{stem}.dec{k}"))).collect::<Result<_, String>>()?,
                    out_weight: t4(&format!("{stem}.out.weight"))?,
                    out_bias: t1(&format!("{stem}.out.bias"))?,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(Net { nets, device: device.clone() })
    }
}

impl super::Net for Net {
    fn estimate(&self, magnitudes: Vec<f32>) -> Vec<f32> {
        let x = Tensor::<4>::from_data(TensorData::new(magnitudes, [1, 2, T, F]), &self.device);
        let estimates: Vec<Tensor<4>> = self.nets.iter().map(|n| n.forward(x.clone())).collect();
        Tensor::cat(estimates, 0).into_data().try_into_vec::<f32>().expect("float estimates")
    }
}
