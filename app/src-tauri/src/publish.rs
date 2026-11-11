//! Sends published notes to the blog server (see `server/` in this repo).
//! Only notes flagged `published` are ever sent; everything else stays local.

use std::time::Duration;

use coda_core::Note;
use serde::Deserialize;

const TIMEOUT: Duration = Duration::from_secs(20);

/// The `[server]` section of config.toml.
#[derive(Debug, Clone, Deserialize)]
pub struct Server {
    pub url: String,
    pub token: String,
}

#[derive(Deserialize)]
struct Published {
    url: String,
}

impl Server {
    fn agent() -> ureq::Agent {
        ureq::Agent::config_builder().timeout_global(Some(TIMEOUT)).build().into()
    }

    fn endpoint(&self, id: &str) -> String {
        format!("{}/api/posts/{id}", self.url.trim_end_matches('/'))
    }

    fn bearer(&self) -> String {
        format!("Bearer {}", self.token)
    }

    /// Publish or update a note. Returns the post's public address.
    pub fn push(&self, note: &Note) -> Result<String, String> {
        let summary = &note.summary;
        let id = summary.id.as_deref().ok_or("this note has no id yet")?;
        let slug = summary.path.rsplit('/').next().unwrap_or("").trim_end_matches(".md");
        let mut response = Self::agent()
            .put(self.endpoint(id))
            .header("Authorization", self.bearer())
            .send_json(serde_json::json!({
                "slug": slug,
                "title": summary.title,
                "body": note.body,
                "date": summary.date,
                "updated": summary.updated,
            }))
            .map_err(|e| self.explain(e))?;
        let published: Published = response.body_mut().read_json().map_err(|e| self.explain(e))?;
        Ok(published.url)
    }

    /// Take a note off the blog.
    pub fn remove(&self, id: &str) -> Result<(), String> {
        Self::agent()
            .delete(self.endpoint(id))
            .header("Authorization", self.bearer())
            .call()
            .map_err(|e| self.explain(e))?;
        Ok(())
    }

    fn explain(&self, error: ureq::Error) -> String {
        match error {
            ureq::Error::StatusCode(401) => {
                format!("{} rejected the token in config.toml", self.url)
            }
            ureq::Error::StatusCode(503) => {
                format!("{} has no CODA_TOKEN set, so it cannot accept posts", self.url)
            }
            ureq::Error::StatusCode(code) => format!("{} answered with error {code}", self.url),
            other => format!("could not reach {}: {other}", self.url),
        }
    }
}
