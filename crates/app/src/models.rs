//! The engine's models (stem separation, the listening for lyrics), downloaded the first time one is needed.

const URL: &str = "https://github.com/winnerspiros/djsugar/releases/download/stem-models";
/// Where the model that listens for words is published, at the revision the engine knows the hashes of.
const HEARS: &str =
    "https://huggingface.co/onnx-community/whisper-base_timestamped/resolve/608c49e61301901684bc36cac8f74b95ff6b5a8e";

/// Where the model file `name` is downloaded from.
fn source(name: &str) -> String {
    match name {
        "whisper-base-encoder.onnx" => format!("{HEARS}/onnx/encoder_model.onnx"),
        "whisper-base-decoder.onnx" => format!("{HEARS}/onnx/decoder_model_merged.onnx"),
        "whisper-base-tokens.json" => format!("{HEARS}/tokenizer.json"),
        _ => format!("{URL}/{name}"),
    }
}

/// Lets the engine fetch a model it lacks, checked against the hash it knows it by.
pub fn install() {
    djsugar_engine::separation::set_fetch(Box::new(|name, hash, to| {
        sugar_net::http::download(&source(name), &[], to, None, 4, &|_, _| true).map_err(|e| e.to_string())?;
        let got = sugar_net::http::sha256(to).map_err(|e| e.to_string())?;
        if got == hash { Ok(()) } else { Err("the download is damaged".into()) }
    }));
}
