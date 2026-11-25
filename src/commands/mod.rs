//! Command dispatch for ApkModder.

mod add_smali;
mod append_method;
mod discover_method;
mod extract_entry;
mod insert_after;
mod insert_before;
mod multi_patch;
mod open;
mod patch;
mod put_entry;
mod remove_lines;
mod replace_bytes_global;
mod replace_text;
mod replace_text_class;
mod replace_text_global;
mod discover_text;
mod extract_libs;
mod extract_resource_all;
mod extract_sig;
mod sig_info;
mod view;

#[cfg(feature = "gui")]
mod gui;

use crate::cli::Commands;
use crate::cli::GlobalOpts;
use crate::error::Result;
use crate::pipeline::patch_flow;
use std::io::Write;

pub fn run(cmd: Commands, _global_opts: &GlobalOpts) -> Result<()> {
    match cmd {
        Commands::Open { apk } => open::run(&apk),
        Commands::View {
            apk,
            class,
            method,
            format,
        } => view::run(&apk, &class, method.as_deref(), &format),
        Commands::Patch {
            input,
            output,
            class,
            method,
            smali_file,
            smali_body,
            sig_dir,
            no_sign,
        } => patch::run(
            &input,
            &output,
            &class,
            &method,
            smali_file.as_deref(),
            smali_body.as_deref(),
            sig_dir.as_deref(),
            no_sign,
        ),
        Commands::MultiPatch {
            input,
            output,
            patch,
            patch_body,
            sig_dir,
            no_sign,
        } => multi_patch::run(
            &input,
            &output,
            &patch,
            &patch_body,
            sig_dir.as_deref(),
            no_sign,
        ),
        Commands::ReplaceText {
            input,
            output,
            class,
            method,
            old,
            new,
            no_sign,
        } => replace_text::run(
            &input,
            &output,
            &class,
            &method,
            &old,
            &new,
            no_sign,
        ),
        Commands::InsertAfter {
            input,
            output,
            class,
            method,
            anchor,
            text,
            no_sign,
        } => insert_after::run(
            &input,
            &output,
            &class,
            &method,
            &anchor,
            &text,
            no_sign,
        ),
        Commands::InsertBefore {
            input,
            output,
            class,
            method,
            anchor,
            text,
            no_sign,
        } => insert_before::run(
            &input,
            &output,
            &class,
            &method,
            &anchor,
            &text,
            no_sign,
        ),
        Commands::RemoveLines {
            input,
            output,
            class,
            method,
            pattern,
            no_sign,
        } => remove_lines::run(
            &input,
            &output,
            &class,
            &method,
            &pattern,
            no_sign,
        ),
        Commands::AppendMethod {
            input,
            output,
            class,
            method_decl,
            smali_body,
            no_sign,
        } => append_method::run(
            &input,
            &output,
            &class,
            &method_decl,
            &smali_body,
            no_sign,
        ),
        Commands::ReplaceTextClass {
            input,
            output,
            class,
            old,
            new,
            no_sign,
        } => replace_text_class::run(
            &input,
            &output,
            &class,
            &old,
            &new,
            no_sign,
        ),
        Commands::ReplaceTextGlobal {
            input,
            output,
            old,
            new,
            no_sign,
        } => replace_text_global::run(&input, &output, &old, &new, no_sign),
        Commands::ReplaceBytesGlobal {
            input,
            output,
            old_hex,
            new_hex,
            no_sign,
        } => {
            let sig_dir = if no_sign { None } else { None };
            let result = patch_flow::replace_bytes_global(
                &input,
                &output,
                &old_hex,
                &new_hex,
                sig_dir,
            )?;
            println!(
                "Replaced bytes globally in {} (unsigned, no signatures).",
                input
            );
            println!("Output: {}", result.output_path);
            Ok(())
        }
        Commands::ExtractEntry {
            input,
            entry,
            output,
        } => extract_entry::run(&input, &entry, &output),
        Commands::PutEntry {
            input,
            output,
            entry,
            input_file,
            ..
        } => put_entry::run(&input, &output, &entry, &input_file),
        Commands::AddSmali {
            input,
            output,
            smali_file,
            no_sign,
        } => add_smali::run(&input, &output, &smali_file, no_sign),
        Commands::ExtractSmaliAll {
            input,
            output_dir,
        } => {
            let count = patch_flow::extract_smali_all(&input, &output_dir)?;
            println!("Extracted {count} smali classes from {input} -> {output_dir}");
            Ok(())
        }
        Commands::InjectSmaliAll {
            input,
            output,
            smali_dir,
            no_sign,
        } => {
            let sig_dir = if no_sign { None } else { None };
            let result =
                patch_flow::inject_smali_all(&input, &output, &smali_dir, sig_dir)?;
            println!(
                "Injected smali classes into {} (unsigned, no signatures).",
                input
            );
            println!("Output: {}", result.output_path);
            Ok(())
        }
        Commands::ExtractSmaliClass {
            input,
            class,
            output,
        } => {
            patch_flow::extract_smali_class(&input, &class, &output)?;
            println!("Extracted class {class} -> {output}");
            Ok(())
        }
        Commands::InjectSmaliClass {
            input,
            output,
            class,
            smali_file,
            no_sign,
        } => {
            let sig_dir = if no_sign { None } else { None };
            let result = patch_flow::inject_smali_class(
                &input,
                &output,
                &class,
                &smali_file,
                sig_dir,
            )?;
            println!(
                "Injected class {class} into {} (unsigned, no signatures).",
                input
            );
            println!("Output: {}", result.output_path);
            Ok(())
        }
        Commands::ExtractResource {
            input,
            resource,
            output,
        } => {
            let mut file = std::fs::File::open(&input)
                .map_err(|e| crate::error::Error::Io(e))?;
            let mut apk = apkscope_core::apk::Apk::new(&mut file)
                .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;
            let data = apk.file(&resource).map_err(|e| {
                crate::error::Error::Other(format!("Failed to read resource {resource}: {e}"))
            })?;
            match data {
                Some(data) => {
                    std::fs::write(&output, &data).map_err(|e| crate::error::Error::Io(e))?;
                    println!("Extracted resource {resource} -> {output}");
                }
                None => {
                    return Err(crate::error::Error::Other(format!(
                        "Resource not found: {resource}"
                    )));
                }
            }
            Ok(())
        }
        Commands::PutResource {
            input,
            output,
            resource,
            input_file,
            no_sign,
        } => {
            let _no_sign = no_sign;
            let data = std::fs::read(&input_file).map_err(|e| crate::error::Error::Io(e))?;

            let mut file = std::fs::File::open(&input)
                .map_err(|e| crate::error::Error::Io(e))?;
            let mut apk = apkscope_core::apk::Apk::new(&mut file)
                .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;
            apk.put(&resource, data).map_err(|e| {
                crate::error::Error::Other(format!("Failed to put resource {resource}: {e}"))
            })?;

            let _edits = apk.edits().map_err(|e| {
                crate::error::Error::Other(format!("Failed to write APK edits: {e}"))
            })?;

            let temp_output = format!("{output}.tmp");
            {
                let mut out_file = std::fs::File::create(&temp_output)
                    .map_err(|e| crate::error::Error::Io(e))?;
                let _bytes_written = apk.rebuild(&mut out_file).map_err(|e| {
                    crate::error::Error::Rebuild(e.to_string())
                })?;
                out_file.flush().map_err(|e| crate::error::Error::Io(e))?;
            }

            std::fs::rename(&temp_output, &output).map_err(|e| crate::error::Error::Io(e))?;

            println!(
                "Put resource {resource} into {} (unsigned, no signatures).",
                input
            );
            println!("Output: {}", output);
            Ok(())
        }
        Commands::ExtractSig { apk, output_dir } => {
            extract_sig::run(&apk, &output_dir)
        }
        Commands::SigInfo { apk } => sig_info::run(&apk),
        #[cfg(feature = "gui")]
        Commands::Gui { apk } => gui::run(apk),
        Commands::DiscoverMethod { input, method } => {
            discover_method::run(&input, &method)
        }
        Commands::DiscoverText { input, text } => {
            discover_text::run(&input, &text)
        }
        Commands::ExtractLibs { input, output_dir } => {
            extract_libs::run(&input, &output_dir)
        }
        Commands::ExtractResourceAll {
            input,
            pattern,
            output_dir,
        } => extract_resource_all::run(&input, &pattern, &output_dir),
    }
}
