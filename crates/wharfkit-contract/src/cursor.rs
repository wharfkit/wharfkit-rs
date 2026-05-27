use crate::{Table, TableError};
use antelope::api::client::{APIClient, DefaultProvider};
use antelope::api::v1::structs::TableIndexType;
use antelope::serializer::Packer;
use std::sync::Arc;

pub struct TableCursor<R: Packer + Default> {
    table: Arc<Table<R>>,
    client: Arc<APIClient<DefaultProvider>>,
    rows_per_request: u32,
    next_key: Option<TableIndexType>,
    done: bool,
}

impl<R: Packer + Default> TableCursor<R> {
    pub fn new(
        table: Arc<Table<R>>,
        client: Arc<APIClient<DefaultProvider>>,
        rows_per_request: u32,
    ) -> Self {
        Self {
            table,
            client,
            rows_per_request,
            next_key: None,
            done: false,
        }
    }

    pub async fn next_page(&mut self) -> Result<Vec<R>, TableError> {
        if self.done {
            return Ok(vec![]);
        }
        let params = self
            .table
            .params(self.next_key.take(), None, self.rows_per_request);
        let resp = self
            .client
            .v1_chain
            .get_table_rows::<R>(params)
            .await
            .map_err(TableError::GetTableRows)?;
        if resp.more {
            self.next_key = resp.next_key;
        } else {
            self.done = true;
        }
        Ok(resp.rows)
    }
}
