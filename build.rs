use std::process::{Command, Stdio};
use std::path::Path;

fn main() -> anyhow::Result<()> {
	println!("cargo:rerun-if-changed=build.rs");

	#[cfg(windows)]
	let glslc_path = {
		let sdk_path = std::env::var("VULKAN_SDK").expect("Couldn't read VULKAN_SDK");
		Path::new(&sdk_path).join("Bin/glslc.exe")
	};

	#[cfg(not(windows))]
	let glslc_path = Path::new("glslc");

	for entry in std::fs::read_dir("shaders")? {
		let entry = entry?;
		if !entry.file_type()?.is_file() {
			continue
		}

		let shader_path = entry.path();
		let extension = shader_path.extension().unwrap();
		if extension == "spv" {
			continue
		}

		compile_glsl(&glslc_path, &shader_path)?;
	}

	Ok(())
}

fn compile_glsl(compiler_path: &Path, shader_path: &Path) -> anyhow::Result<()> {
	// use std::io::Write;

	println!("cargo:rerun-if-changed={}", shader_path.display());

	let extension = shader_path.extension().unwrap();
	if extension != "glsl" {
		anyhow::bail!("Shader extension must be '.glsl' - got '{}'", shader_path.display())
	}

	let without_glsl = shader_path.with_extension("");
	let Some(second_extension) = without_glsl.extension() else {
		println!("Skipping '{}' as it doesn't have a secondary extension", shader_path.display());
		return Ok(())
	};

	let shader_stage = match second_extension.to_str() {
		Some("vs") => "vertex",
		Some("fs") => "fragment",
		Some("cs") => "compute",
		_ => anyhow::bail!("Unknown secondary extension '{second_extension:?}' in {}", shader_path.display()),
	};

	let mut target_path = without_glsl;
	target_path.as_mut_os_string().push(".spv");

	println!("cargo:rerun-if-changed={}", target_path.display());

	Command::new(compiler_path)
		.stdout(Stdio::inherit())
		.stderr(std::io::stdout())
		.arg(&format!("-fshader-stage={shader_stage}"))
		.arg("--target-env=vulkan1.3")
		.arg(shader_path)
		.arg("-o")
		.arg(target_path)
		.spawn()?
		.wait()?;

	Ok(())
}
