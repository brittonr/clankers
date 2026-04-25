//! Full-text search index over session message content using tantivy.
//!
//! Stores an inverted index at `~/.clankers/agent/search_index/` with
//! BM25 ranking, phrase queries, and per-session grouping.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use std::path::Path;
use std::path::PathBuf;

use tantivy::Index;
use tantivy::IndexReader;
use tantivy::IndexWriter;
use tantivy::ReloadPolicy;
use tantivy::Searcher;
use tantivy::collector::Count;
use tantivy::collector::TopDocs;
use tantivy::directory::MmapDirectory;
use tantivy::doc;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::snippet::SnippetGenerator;

use crate::error::Result;

const INDEX_HEAP_SIZE: usize = 15_000_000; // 15MB writer heap

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub session_id: String,
    pub message_id: String,
    pub role: String,
    pub score: f32,
    pub snippet: String,
    pub timestamp: i64,
}

pub struct SearchIndex {
    index: Index,
    reader: IndexReader,
    schema: Schema,
    path: PathBuf,
    // Field handles
    f_session_id: Field,
    f_message_id: Field,
    f_role: Field,
    f_content: Field,
    f_timestamp: Field,
}

impl SearchIndex {
    pub fn open(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path).map_err(|e| crate::error::DbError {
            message: format!("failed to create search index directory: {e}"),
        })?;

        let mut schema_builder = Schema::builder();
        let f_session_id = schema_builder.add_text_field("session_id", STRING | STORED);
        let f_message_id = schema_builder.add_text_field("message_id", STRING | STORED);
        let f_role = schema_builder.add_text_field("role", STRING | STORED);
        let f_content = schema_builder.add_text_field("content", TEXT | STORED);
        let f_timestamp = schema_builder.add_i64_field("timestamp", INDEXED | STORED);
        let schema = schema_builder.build();

        let dir = MmapDirectory::open(path).map_err(|e| crate::error::DbError {
            message: format!("failed to open search index directory: {e}"),
        })?;

        let index = Index::open_or_create(dir, schema.clone()).map_err(|e| crate::error::DbError {
            message: format!("failed to open search index: {e}"),
        })?;

        let reader = index.reader_builder().reload_policy(ReloadPolicy::OnCommitWithDelay).try_into().map_err(|e| {
            crate::error::DbError {
                message: format!("failed to create index reader: {e}"),
            }
        })?;

        Ok(Self {
            index,
            reader,
            schema,
            path: path.to_path_buf(),
            f_session_id,
            f_message_id,
            f_role,
            f_content,
            f_timestamp,
        })
    }

    pub fn index_message(
        &self,
        session_id: &str,
        message_id: &str,
        role: &str,
        content: &str,
        timestamp: i64,
    ) -> Result<()> {
        if content.trim().is_empty() {
            return Ok(());
        }

        let mut writer = self.writer()?;
        writer
            .add_document(tantivy::doc!(
                self.f_session_id => session_id,
                self.f_message_id => message_id,
                self.f_role => role,
                self.f_content => content,
                self.f_timestamp => timestamp,
            ))
            .map_err(|e| crate::error::DbError {
                message: format!("failed to add document: {e}"),
            })?;
        writer.commit().map_err(|e| crate::error::DbError {
            message: format!("failed to commit index: {e}"),
        })?;
        Ok(())
    }

    pub fn index_messages_batch(&self, messages: &[(&str, &str, &str, &str, i64)]) -> Result<u64> {
        if messages.is_empty() {
            return Ok(0);
        }

        let mut writer = self.writer()?;
        let mut count = 0u64;

        for &(session_id, message_id, role, content, timestamp) in messages {
            if content.trim().is_empty() {
                continue;
            }
            writer
                .add_document(tantivy::doc!(
                    self.f_session_id => session_id,
                    self.f_message_id => message_id,
                    self.f_role => role,
                    self.f_content => content,
                    self.f_timestamp => timestamp,
                ))
                .map_err(|e| crate::error::DbError {
                    message: format!("failed to add document: {e}"),
                })?;
            count += 1;
        }

        writer.commit().map_err(|e| crate::error::DbError {
            message: format!("failed to commit batch index: {e}"),
        })?;
        Ok(count)
    }

    pub fn search(&self, query_str: &str, limit: usize) -> Result<Vec<SearchHit>> {
        if query_str.trim().is_empty() {
            return Ok(Vec::new());
        }

        let searcher = self.searcher();
        let query_parser = QueryParser::for_index(&self.index, vec![self.f_content]);
        let query = query_parser.parse_query(query_str).map_err(|e| crate::error::DbError {
            message: format!("failed to parse search query: {e}"),
        })?;

        let collector = TopDocs::with_limit(limit).order_by_score();
        let top_docs = searcher.search(&query, &collector).map_err(|e| crate::error::DbError {
            message: format!("search failed: {e}"),
        })?;

        let snippet_gen =
            SnippetGenerator::create(&searcher, &query, self.f_content).map_err(|e| crate::error::DbError {
                message: format!("failed to create snippet generator: {e}"),
            })?;

        let mut hits = Vec::with_capacity(top_docs.len());
        for (score, doc_address) in top_docs {
            let doc = searcher.doc::<TantivyDocument>(doc_address).map_err(|e| crate::error::DbError {
                message: format!("failed to retrieve document: {e}"),
            })?;

            let session_id = doc.get_first(self.f_session_id).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let message_id = doc.get_first(self.f_message_id).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let role = doc.get_first(self.f_role).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let timestamp = doc.get_first(self.f_timestamp).and_then(|v| v.as_i64()).unwrap_or(0);

            let snippet = snippet_gen.snippet_from_doc(&doc);
            let snippet_text = snippet.to_html();

            hits.push(SearchHit {
                session_id,
                message_id,
                role,
                score,
                snippet: snippet_text,
                timestamp,
            });
        }

        Ok(hits)
    }

    pub fn has_session(&self, session_id: &str) -> Result<bool> {
        let searcher = self.searcher();
        let term = tantivy::Term::from_field_text(self.f_session_id, session_id);
        let query = tantivy::query::TermQuery::new(term, IndexRecordOption::Basic);
        let count = searcher.search(&query, &Count).map_err(|e| crate::error::DbError {
            message: format!("session check failed: {e}"),
        })?;
        Ok(count > 0)
    }

    pub fn document_count(&self) -> Result<u64> {
        let searcher = self.searcher();
        Ok(searcher.num_docs())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn writer(&self) -> Result<IndexWriter> {
        self.index.writer(INDEX_HEAP_SIZE).map_err(|e| crate::error::DbError {
            message: format!("failed to create index writer: {e}"),
        })
    }

    fn searcher(&self) -> Searcher {
        self.reader.searcher()
    }
}

/// Progress callback for backfill operations.
pub type BackfillProgress = Box<dyn Fn(u32, u32) + Send>;

/// Backfill result summary.
#[derive(Debug, Clone)]
pub struct BackfillResult {
    pub sessions_processed: u32,
    pub messages_indexed: u64,
    pub sessions_skipped: u32,
    pub errors: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_index() -> (tempfile::TempDir, SearchIndex) {
        let tmp = tempfile::TempDir::new().unwrap();
        let idx = SearchIndex::open(tmp.path()).unwrap();
        (tmp, idx)
    }

    #[test]
    fn index_and_search() {
        let (_tmp, idx) = test_index();

        idx.index_message("s1", "m1", "user", "fix the authentication bug in login", 1000).unwrap();
        idx.index_message("s1", "m2", "assistant", "I found the issue in the auth module", 1001).unwrap();
        idx.index_message("s2", "m3", "user", "refactor the database layer", 2000).unwrap();

        // Force reload
        idx.reader.reload().unwrap();

        let hits = idx.search("authentication", 10).unwrap();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].session_id, "s1");
    }

    #[test]
    fn empty_query() {
        let (_tmp, idx) = test_index();
        let hits = idx.search("", 10).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn empty_content_skipped() {
        let (_tmp, idx) = test_index();
        idx.index_message("s1", "m1", "user", "   ", 1000).unwrap();
        idx.reader.reload().unwrap();
        assert_eq!(idx.document_count().unwrap(), 0);
    }

    #[test]
    fn batch_indexing() {
        let (_tmp, idx) = test_index();
        let messages = vec![
            ("s1", "m1", "user", "implement the search feature", 1000i64),
            ("s1", "m2", "assistant", "I'll add tantivy search", 1001),
            ("s2", "m3", "user", "write tests for search", 2000),
        ];
        let count = idx.index_messages_batch(&messages).unwrap();
        assert_eq!(count, 3);

        idx.reader.reload().unwrap();
        let hits = idx.search("search", 10).unwrap();
        assert!(hits.len() >= 2);
    }

    #[test]
    fn has_session_check() {
        let (_tmp, idx) = test_index();
        idx.index_message("sess-abc", "m1", "user", "hello world", 1000).unwrap();
        idx.reader.reload().unwrap();
        assert!(idx.has_session("sess-abc").unwrap());
        assert!(!idx.has_session("nonexistent").unwrap());
    }

    #[test]
    fn ranking_by_relevance() {
        let (_tmp, idx) = test_index();

        idx.index_message("s1", "m1", "user", "rust programming language", 1000).unwrap();
        idx.index_message("s2", "m2", "user", "rust rust rust programming in rust", 2000).unwrap();

        idx.reader.reload().unwrap();

        let hits = idx.search("rust", 10).unwrap();
        assert!(hits.len() >= 2);
        assert!(hits[0].score >= hits[1].score, "higher term frequency should rank higher");
    }
}
