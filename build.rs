use std::process::Command;
use std::path::Path;
use std::env;

fn main() -> anyhow::Result<()> {
	println!("cargo:rerun-if-changed=build.rs");

	let sdk_path = env::var("VULKAN_SDK").expect("Couldn't read VULKAN_SDK");

	println!("{sdk_path:?}");

	let compiler_path = Path::new(&sdk_path).join("Bin/glslc.exe");
	println!("{compiler_path:?}");

	for entry in std::fs::read_dir("shaders")? {
		let entry = entry?;
		if !entry.file_type()?.is_file() {
			continue
		}

		let shader_path = entry.path();
		if shader_path.extension().unwrap() == "spv" {
			continue
		}

		compile(&compiler_path, &shader_path)?;
	}
	
	Ok(())
}

fn compile(compiler_path: &Path, shader_path: &Path) -> anyhow::Result<()> {
	println!("cargo:rerun-if-changed={}", shader_path.display());

	let mut extension = shader_path.extension().unwrap().to_owned();
	extension.push(".spv");

	let target_path = shader_path.with_extension(extension);
	println!("cargo:rerun-if-changed={}", target_path.display());

	Command::new(compiler_path)
		.arg(shader_path)
		.arg("-o")
		.arg(target_path)
		.output()?;

	Ok(())
}