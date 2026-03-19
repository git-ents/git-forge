//! `git2::Repository` implementation of [`Contributors`].

use git2::Repository;

use crate::contributor::{CONTRIBUTORS_REF, Contributor, Contributors};

fn parse_meta(content: &str) -> Option<(String, String)> {
    let mut name = None;
    let mut email = None;
    for line in content.lines() {
        if let Some(val) = line.strip_prefix("name = \"") {
            name = Some(val.trim_end_matches('"').to_string());
        } else if let Some(val) = line.strip_prefix("email = \"") {
            email = Some(val.trim_end_matches('"').to_string());
        }
    }
    match (name, email) {
        (Some(n), Some(e)) => Some((n, e)),
        _ => None,
    }
}

fn contributor_from_tree(
    repo: &Repository,
    root: &git2::Tree<'_>,
    id: &str,
) -> Result<Option<Contributor>, git2::Error> {
    let Some(entry) = root.get_name(id) else {
        return Ok(None);
    };
    let obj = entry.to_object(repo)?;
    let subtree = obj
        .as_tree()
        .ok_or_else(|| git2::Error::from_str(&format!("contributor entry '{id}' is not a tree")))?;
    let Some(meta_entry) = subtree.get_name("meta") else {
        return Ok(None);
    };
    let meta_obj = meta_entry.to_object(repo)?;
    let blob = meta_obj
        .as_blob()
        .ok_or_else(|| git2::Error::from_str("contributor meta is not a blob"))?;
    let content = std::str::from_utf8(blob.content()).unwrap_or("");
    let Some((name, email)) = parse_meta(content) else {
        return Ok(None);
    };
    Ok(Some(Contributor {
        id: id.to_string(),
        name,
        email,
    }))
}

impl Contributors for Repository {
    fn list_contributors(&self) -> Result<Vec<Contributor>, git2::Error> {
        let reference = match self.find_reference(CONTRIBUTORS_REF) {
            Ok(r) => r,
            Err(e) if e.code() == git2::ErrorCode::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let tree = reference.peel_to_commit()?.tree()?;
        let mut contributors = Vec::new();
        for entry in tree.iter() {
            let Some(id) = entry.name() else { continue };
            if let Some(c) = contributor_from_tree(self, &tree, id)? {
                contributors.push(c);
            }
        }
        contributors.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(contributors)
    }

    fn find_contributor(&self, id: &str) -> Result<Option<Contributor>, git2::Error> {
        let reference = match self.find_reference(CONTRIBUTORS_REF) {
            Ok(r) => r,
            Err(e) if e.code() == git2::ErrorCode::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let tree = reference.peel_to_commit()?.tree()?;
        contributor_from_tree(self, &tree, id)
    }

    fn find_contributor_by_email(&self, email: &str) -> Result<Option<Contributor>, git2::Error> {
        Ok(self
            .list_contributors()?
            .into_iter()
            .find(|c| c.email == email))
    }

    fn add_contributor(&self, id: &str, name: &str, email: &str) -> Result<(), git2::Error> {
        let existing_commit = match self.find_reference(CONTRIBUTORS_REF) {
            Ok(r) => Some(r.peel_to_commit()?),
            Err(e) if e.code() == git2::ErrorCode::NotFound => None,
            Err(e) => return Err(e),
        };

        let existing_tree = existing_commit.as_ref().map(|c| c.tree()).transpose()?;

        if let Some(ref tree) = existing_tree {
            if tree.get_name(id).is_some() {
                return Err(git2::Error::from_str(&format!(
                    "contributor '{id}' already exists"
                )));
            }
        }

        let meta_content = format!("name = \"{name}\"\nemail = \"{email}\"\n");
        let meta_blob = self.blob(meta_content.as_bytes())?;

        let contributor_tree_oid = {
            let mut tb = self.treebuilder(None)?;
            tb.insert("meta", meta_blob, 0o100_644)?;
            tb.write()?
        };

        let root_tree_oid = {
            let mut tb = self.treebuilder(existing_tree.as_ref())?;
            tb.insert(id, contributor_tree_oid, 0o040_000)?;
            tb.write()?
        };

        let tree = self.find_tree(root_tree_oid)?;
        let sig = self.signature()?;
        let message = format!("add contributor {id}");
        let parents: &[&git2::Commit<'_>] = match existing_commit.as_ref() {
            Some(c) => &[c],
            None => &[],
        };
        self.commit(Some(CONTRIBUTORS_REF), &sig, &sig, &message, &tree, parents)?;

        Ok(())
    }
}
