//! Execution logic for `git forge contributor`.

use std::process;

use git2::Repository;
use git_forge::cli::ContributorSubcommand;
use git_forge_core::contributor::Contributors;

fn derive_id(name: &str) -> String {
    name.split_whitespace()
        .next()
        .unwrap_or("contributor")
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect()
}

fn run_inner(command: ContributorSubcommand) -> Result<(), Box<dyn std::error::Error>> {
    let repo = Repository::open_from_env()?;

    match command {
        ContributorSubcommand::Add { id, name, emails } => {
            let cfg = repo.config()?;
            let name = name
                .or_else(|| cfg.get_string("user.name").ok())
                .ok_or("no name provided; set user.name in git config or pass --name")?;
            let emails = if emails.is_empty() {
                cfg.get_string("user.email")
                    .ok()
                    .map(|e| vec![e])
                    .ok_or("no email provided; set user.email in git config or pass --email")?
            } else {
                emails
            };
            let id = id.unwrap_or_else(|| derive_id(&name));
            repo.add_contributor(&id, &name, &emails)?;
            eprintln!("Added contributor {id} ({name} <{}>).", emails.join(", "));
        }

        ContributorSubcommand::Edit { id, new_id, name, add_emails, remove_emails } => {
            if new_id.is_none() && name.is_none() && add_emails.is_empty() && remove_emails.is_empty() {
                return Err("nothing to update; pass --rename-id, --name, --add-email, or --remove-email".into());
            }
            repo.update_contributor(&id, new_id.as_deref(), name.as_deref(), &add_emails, &remove_emails)?;
            eprintln!("Updated contributor {id}.");
        }

        ContributorSubcommand::List => {
            let contributors = repo.list_contributors()?;
            if contributors.is_empty() {
                println!("No contributors registered.");
            } else {
                for c in &contributors {
                    println!("{}\t{} <{}>", c.id, c.name, c.emails.join(", "));
                }
            }
        }

        ContributorSubcommand::Remove { id } => {
            repo.remove_contributor(&id)?;
            eprintln!("Removed contributor {id}.");
        }

        ContributorSubcommand::Show { id } => {
            match repo.find_contributor(&id)? {
                None => {
                    eprintln!("Contributor '{id}' not found.");
                    process::exit(1);
                }
                Some(c) => {
                    println!("ID:     {}", c.id);
                    println!("Name:   {}", c.name);
                    for email in &c.emails {
                        println!("Email:  {email}");
                    }
                }
            }
        }
    }

    Ok(())
}

/// Execute a `contributor` subcommand.
pub fn run(command: ContributorSubcommand) {
    if let Err(e) = run_inner(command) {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}
