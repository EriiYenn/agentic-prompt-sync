use crate::error::{ApsError, Result};
use git2::{FetchOptions, RemoteCallbacks, Repository};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tracing::{debug, info};

/// Result of resolving a git source
pub struct ResolvedGitSource {
    /// Temp directory containing the clone (must be kept alive)
    pub temp_dir: TempDir,
    /// Path to the cloned repository
    pub repo_path: PathBuf,
    /// Resolved ref name (e.g., "main", "master", or the original ref)
    pub resolved_ref: String,
    /// Commit SHA at the resolved ref
    pub commit_sha: String,
}

/// Clone a git repository and resolve the ref
pub fn clone_and_resolve(url: &str, git_ref: &str, shallow: bool) -> Result<ResolvedGitSource> {
    info!("Cloning git repository: {}", url);

    // Create temp directory for the clone
    let temp_dir = TempDir::new()
        .map_err(|e| ApsError::io(e, "Failed to create temp directory for git clone"))?;

    let repo_path = temp_dir.path().to_path_buf();

    // Determine if this is an SSH URL (needs credentials)
    let is_ssh = url.starts_with("git@") || url.starts_with("ssh://");

    // For shallow clones with auto ref, we need to try different branches
    let refs_to_try = if git_ref == "auto" {
        vec!["main", "master"]
    } else {
        vec![git_ref]
    };

    let (repo, resolved_ref) = clone_with_ref_fallback(url, &repo_path, &refs_to_try, shallow, is_ssh)?;

    // Get the commit SHA
    let head = repo.head().map_err(|e| ApsError::GitError {
        message: format!("Failed to get HEAD: {}", e),
    })?;

    let commit_sha = head
        .peel_to_commit()
        .map_err(|e| ApsError::GitError {
            message: format!("Failed to get commit: {}", e),
        })?
        .id()
        .to_string();

    info!(
        "Cloned {} at ref '{}' (commit {})",
        url, resolved_ref, &commit_sha[..8]
    );

    Ok(ResolvedGitSource {
        temp_dir,
        repo_path,
        resolved_ref,
        commit_sha,
    })
}

/// Try to clone with fallback refs
fn clone_with_ref_fallback(
    url: &str,
    path: &Path,
    refs: &[&str],
    shallow: bool,
    is_ssh: bool,
) -> Result<(Repository, String)> {
    let mut last_error = None;

    for ref_name in refs {
        debug!("Trying to clone with ref '{}'", ref_name);

        // Clean up any previous failed attempt
        if path.exists() {
            let _ = std::fs::remove_dir_all(path);
        }

        // Create fresh builder and fetch options for each attempt
        let mut builder = git2::build::RepoBuilder::new();
        let mut fetch_opts = FetchOptions::new();

        // Only add credentials callback for SSH URLs
        if is_ssh {
            let mut callbacks = RemoteCallbacks::new();
            callbacks.credentials(|_url, username_from_url, _allowed_types| {
                git2::Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
            });
            fetch_opts.remote_callbacks(callbacks);
        }

        if shallow {
            fetch_opts.depth(1);
        }

        builder.fetch_options(fetch_opts);
        builder.branch(ref_name);

        match builder.clone(url, path) {
            Ok(repo) => {
                return Ok((repo, ref_name.to_string()));
            }
            Err(e) => {
                debug!("Failed to clone with ref '{}': {}", ref_name, e);
                last_error = Some(e);
            }
        }
    }

    // All refs failed - include the last error in the message
    let error_detail = last_error
        .map(|e| format!(": {}", e))
        .unwrap_or_default();

    Err(ApsError::GitError {
        message: format!(
            "Failed to clone with refs {:?}{}",
            refs.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            error_detail
        ),
    })
}

/// Validate that a path exists in the repository
pub fn validate_path_exists(repo_path: &Path, asset_path: &str) -> Result<PathBuf> {
    let full_path = repo_path.join(asset_path);

    if !full_path.exists() {
        return Err(ApsError::SourcePathNotFound { path: full_path });
    }

    debug!("Validated path exists: {:?}", full_path);
    Ok(full_path)
}

/// Fetch updates to an existing repository
pub fn fetch_and_checkout(repo_path: &Path, git_ref: &str) -> Result<(String, String)> {
    let repo = Repository::open(repo_path).map_err(|e| ApsError::GitError {
        message: format!("Failed to open repository: {}", e),
    })?;

    // Fetch from origin
    let mut remote = repo.find_remote("origin").map_err(|e| ApsError::GitError {
        message: format!("Failed to find remote 'origin': {}", e),
    })?;

    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, _allowed_types| {
        git2::Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"))
    });

    let mut fetch_opts = FetchOptions::new();
    fetch_opts.remote_callbacks(callbacks);

    remote
        .fetch(&[git_ref], Some(&mut fetch_opts), None)
        .map_err(|e| ApsError::GitError {
            message: format!("Failed to fetch: {}", e),
        })?;

    // Get the fetched commit
    let fetch_head = repo.find_reference("FETCH_HEAD").map_err(|e| ApsError::GitError {
        message: format!("Failed to find FETCH_HEAD: {}", e),
    })?;

    let commit = fetch_head.peel_to_commit().map_err(|e| ApsError::GitError {
        message: format!("Failed to get commit: {}", e),
    })?;

    let commit_sha = commit.id().to_string();

    // Checkout the commit
    let obj = commit.into_object();
    repo.checkout_tree(&obj, None).map_err(|e| ApsError::GitError {
        message: format!("Failed to checkout: {}", e),
    })?;

    Ok((git_ref.to_string(), commit_sha))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path_exists() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "test").unwrap();

        // Should succeed for existing file
        let result = validate_path_exists(temp_dir.path(), "test.txt");
        assert!(result.is_ok());

        // Should fail for non-existing file
        let result = validate_path_exists(temp_dir.path(), "nonexistent.txt");
        assert!(result.is_err());
    }
}
