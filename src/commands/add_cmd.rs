use std::path::PathBuf;

use anyhow::anyhow;

use crate::cli_add::{AddArguments, AssetSource, UnityTemplateFile};
use crate::commands::{PERSISTENT_BUILD_SCRIPT_ROOT, add_file_to_project};
use crate::unity::project::ProjectPath;

pub fn add_to_project(args: &AddArguments) -> anyhow::Result<()> {
    if args.display_content {
        println!("{}", args.template.as_asset().load_content()?);
        return Ok(());
    } else if args.display_url {
        return if let AssetSource::Remote(url) = args.template.as_asset().content {
            println!("{url}");
            Ok(())
        } else {
            Err(anyhow!("File does not have a URL source"))
        };
    }

    let project = ProjectPath::try_from(&args.project_dir)?;

    let destination_dir = match args.template {
        UnityTemplateFile::Builder | UnityTemplateFile::BuilderMenu => {
            PathBuf::from(PERSISTENT_BUILD_SCRIPT_ROOT)
        }
        UnityTemplateFile::GitIgnore | UnityTemplateFile::GitAttributes => PathBuf::default(),
    };

    let full_path = project
        .join(&destination_dir)
        .join(args.template.as_asset().filename);

    if full_path.exists() && !args.force {
        return Err(anyhow!(
            "File already exists, add '--force' to overwrite: {}",
            full_path.display()
        ));
    }

    if args.template == UnityTemplateFile::BuilderMenu {
        // The build menu requires the builder script to be added as well.
        let builder_script = UnityTemplateFile::Builder.as_asset();
        let builder_script_path = project.join(&destination_dir).join(builder_script.filename);
        if builder_script_path.exists() {
            println!(
                "Builder script already exists, not adding: {}",
                builder_script_path.display()
            );
        } else {
            add_file_to_project(&project, &destination_dir, UnityTemplateFile::Builder)?;
        }
    }

    add_file_to_project(project, destination_dir, args.template)
}
