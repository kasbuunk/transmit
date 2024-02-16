fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_filepath = "proto/transmit.proto";
    tonic_build::compile_protos(proto_filepath)?;
    Ok(())
}
