use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use tempfile::TempDir;
use verifier::source_storage::{SourceStorageError, git::Git, repository};

const ORIGINAL_VERIFIED_AT: u64 = 1_600_000_000;

#[tokio::test]
async fn repository_check_preserves_clean_and_dirty_storage_without_using_remote()
-> Result<(), Box<dyn Error>> {
    for changes in ["clean", "untracked", "staged", "unstaged"] {
        let storage_root = "verified/contracts";
        let fixture = RepositoryFixture::new(storage_root).await?;
        let git = Git::new(&fixture.repo_path);
        let bundle_path = format!("{storage_root}/bundle.txt");
        let file = fixture.repo_path.join(&bundle_path);
        fs::create_dir_all(file.parent().expect("storage directory"))?;
        fs::write(&file, "committed\n")?;
        git.run(&["add", "--", &bundle_path]).await?;
        git.run_with_author(
            &["commit", "-m", "Fixture bundle", "--", &bundle_path],
            "Verifier",
            "verifier@example.invalid",
            ORIGINAL_VERIFIED_AT,
        )
        .await?;
        let missing_remote = fixture.temp.path().join("unavailable.git");
        git.run(&[
            "remote",
            "add",
            "origin",
            missing_remote.to_str().expect("remote path"),
        ])
        .await?;

        match changes {
            // Recreate the same contents to invalidate cached index stat information.
            "clean" => {
                fs::remove_file(&file)?;
                fs::write(&file, "committed\n")?;
            }
            "untracked" => fs::write(file.with_file_name("pending.txt"), "pending\n")?,
            "staged" => {
                fs::write(&file, "pending\n")?;
                git.run(&["add", "--", &bundle_path]).await?;
            }
            "unstaged" => fs::write(&file, "pending\n")?,
            _ => unreachable!(),
        }
        let index_path = fixture.repo_path.join(".git/index");
        let staged_before = git.output_untrimmed(&["ls-files", "--stage", "-z"]).await?;
        let mut before = repository_files(fixture.temp.path())?;
        // Git may refresh its stat cache; staged file contents and modes must stay intact.
        before.remove(&index_path);
        let result = repository::check(&fixture.repo_path, storage_root).await;
        if changes == "clean" {
            result?;
        } else {
            assert!(
                matches!(
                    result,
                    Err(SourceStorageError::DirtySourceRepository { .. })
                ),
                "{changes}: {result:?}"
            );
        }
        let mut after = repository_files(fixture.temp.path())?;
        after.remove(&index_path);
        assert_eq!(after, before, "{changes}");
        assert_eq!(
            git.output_untrimmed(&["ls-files", "--stage", "-z"]).await?,
            staged_before,
            "{changes}"
        );
        assert!(!missing_remote.exists());
    }
    Ok(())
}

#[tokio::test]
async fn repository_check_does_not_create_missing_checkout() -> Result<(), Box<dyn Error>> {
    let temp = TempDir::new()?;
    let missing = temp.path().join("missing");
    assert!(matches!(
        repository::check(&missing, "sources").await,
        Err(SourceStorageError::InvalidPath { .. })
    ));
    assert_eq!(fs::read_dir(temp.path())?.count(), 0);
    Ok(())
}

#[tokio::test]
async fn repository_check_rejects_invalid_root_attributes_even_when_head_is_fixed()
-> Result<(), Box<dyn Error>> {
    let fixture = RepositoryFixture::new("sources").await?;
    let git = Git::new(&fixture.repo_path);
    let attributes = fixture.repo_path.join(".gitattributes");
    fs::write(&attributes, "sources/** text\n")?;
    git.run(&["add", "--", ".gitattributes"]).await?;
    fixture
        .commit(&["commit", "--amend", "-m", "Invalid root attributes"])
        .await?;
    fs::write(&attributes, "sources/** -text\n")?;
    git.run(&["add", "--", ".gitattributes"]).await?;
    fixture
        .commit(&["commit", "-m", "Restore current attributes"])
        .await?;

    let error = repository::check(&fixture.repo_path, "sources")
        .await
        .expect_err("initial attributes must be valid independently of HEAD");
    assert!(
        matches!(error, SourceStorageError::UnpreparedSourceRepository { message, .. }
        if message == "root `.gitattributes` must contain exactly `sources/** -text`")
    );
    Ok(())
}

#[tokio::test]
async fn repository_check_requires_current_attributes_rule_to_be_present_and_effective()
-> Result<(), Box<dyn Error>> {
    for (attributes, expected) in [
        (
            "sources/** text\n",
            "current `.gitattributes` must contain `sources/** -text`",
        ),
        (
            "sources/** -text\nsources/** text\n",
            "`sources/** -text` is not effective for `sources/.verifier-attributes-check`",
        ),
    ] {
        let fixture = RepositoryFixture::new("sources").await?;
        fs::write(fixture.repo_path.join(".gitattributes"), attributes)?;
        Git::new(&fixture.repo_path)
            .run(&["add", "--", ".gitattributes"])
            .await?;
        fixture
            .commit(&["commit", "-m", "Change current attributes"])
            .await?;

        let error = repository::check(&fixture.repo_path, "sources")
            .await
            .expect_err("current attributes must remain effective");
        assert!(
            matches!(error, SourceStorageError::UnpreparedSourceRepository { message, .. }
            if message == expected)
        );
    }
    Ok(())
}

fn repository_files(path: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>, Box<dyn Error>> {
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            files.extend(repository_files(&entry.path())?);
        } else {
            files.insert(entry.path(), fs::read(entry.path())?);
        }
    }
    Ok(files)
}

struct RepositoryFixture {
    temp: TempDir,
    repo_path: PathBuf,
}

impl RepositoryFixture {
    async fn new(storage_root: &str) -> Result<Self, Box<dyn Error>> {
        let temp = TempDir::new()?;
        let repo_path = temp.path().join("repo");
        fs::create_dir(&repo_path)?;
        let git = Git::new(&repo_path);
        git.run(&["init", "-b", "main"]).await?;
        git.run(&["config", "commit.gpgsign", "false"]).await?;
        fs::write(
            repo_path.join(".gitattributes"),
            format!("{storage_root}/** -text\n"),
        )?;
        git.run(&["add", "--", ".gitattributes"]).await?;
        let fixture = Self { temp, repo_path };
        fixture
            .commit(&["commit", "-m", "Arbitrary initial commit message"])
            .await?;
        Ok(fixture)
    }

    async fn commit(&self, args: &[&str]) -> Result<(), Box<dyn Error>> {
        Git::new(&self.repo_path)
            .run_with_author(
                args,
                "Verifier",
                "verifier@example.invalid",
                ORIGINAL_VERIFIED_AT,
            )
            .await?;
        Ok(())
    }
}
