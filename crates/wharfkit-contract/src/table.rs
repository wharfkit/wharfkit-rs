use antelope::api::client::{APIClient, DefaultProvider};
use antelope::api::v1::structs::{ClientError, GetTableRowsParams, TableIndexType};
use antelope::chain::name::Name;
use antelope::serializer::Packer;
use std::marker::PhantomData;

const ALL_ROWS_LIMIT: u32 = 1000;

#[derive(thiserror::Error, Debug)]
pub enum TableError {
    #[error("get_table_rows failed: {0:?}")]
    GetTableRows(ClientError<()>),
}

pub struct Table<R: Packer + Default> {
    contract: Name,
    table_name: Name,
    scope: Name,
    _marker: PhantomData<R>,
}

impl<R: Packer + Default> Table<R> {
    pub fn new(contract: Name, table_name: Name, scope: Name) -> Self {
        Self {
            contract,
            table_name,
            scope,
            _marker: PhantomData,
        }
    }

    pub fn contract(&self) -> &Name {
        &self.contract
    }

    pub fn table_name(&self) -> &Name {
        &self.table_name
    }

    pub fn scope(&self) -> &Name {
        &self.scope
    }

    pub(crate) fn params(
        &self,
        lower_bound: Option<TableIndexType>,
        upper_bound: Option<TableIndexType>,
        limit: u32,
    ) -> GetTableRowsParams {
        GetTableRowsParams {
            code: self.contract,
            scope: Some(self.scope),
            table: self.table_name,
            lower_bound,
            upper_bound,
            limit: Some(limit),
            reverse: None,
            index_position: None,
            show_payer: None,
        }
    }

    pub async fn get(
        &self,
        primary_key: u64,
        client: &APIClient<DefaultProvider>,
    ) -> Result<Option<R>, TableError> {
        let lower = TableIndexType::UINT64(primary_key);
        let upper = TableIndexType::UINT64(primary_key);
        let resp = client
            .v1_chain
            .get_table_rows::<R>(self.params(Some(lower), Some(upper), 1))
            .await
            .map_err(TableError::GetTableRows)?;
        Ok(resp.rows.into_iter().next())
    }

    pub async fn all(&self, client: &APIClient<DefaultProvider>) -> Result<Vec<R>, TableError> {
        let resp = client
            .v1_chain
            .get_table_rows::<R>(self.params(None, None, ALL_ROWS_LIMIT))
            .await
            .map_err(TableError::GetTableRows)?;
        if resp.more {
            log::warn!(
                "Table::all on {}/{} truncated at {} rows; use TableCursor for full scans",
                self.contract,
                self.table_name,
                ALL_ROWS_LIMIT
            );
        }
        Ok(resp.rows)
    }

    pub async fn first(
        &self,
        client: &APIClient<DefaultProvider>,
    ) -> Result<Option<R>, TableError> {
        let resp = client
            .v1_chain
            .get_table_rows::<R>(self.params(None, None, 1))
            .await
            .map_err(TableError::GetTableRows)?;
        Ok(resp.rows.into_iter().next())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use antelope::chain::asset::Asset;
    use antelope::serializer::Encoder;
    use std::sync::Arc;

    #[derive(Default)]
    struct AccountRow {
        balance: Asset,
    }

    impl Packer for AccountRow {
        fn size(&self) -> usize {
            self.balance.size()
        }
        fn pack(&self, e: &mut Encoder) -> usize {
            self.balance.pack(e)
        }
        fn unpack(&mut self, d: &[u8]) -> usize {
            self.balance.unpack(d)
        }
    }

    fn jungle4_client() -> Arc<APIClient<DefaultProvider>> {
        Arc::new(
            APIClient::<DefaultProvider>::default_provider(
                "https://jungle4.greymass.com".to_string(),
                None,
            )
            .unwrap(),
        )
    }

    fn teamgreymass_eosio_token_accounts() -> Table<AccountRow> {
        Table::<AccountRow>::new(
            Name::new_from_str("eosio.token"),
            Name::new_from_str("accounts"),
            Name::new_from_str("teamgreymass"),
        )
    }

    #[test]
    fn table_construct() {
        let table = teamgreymass_eosio_token_accounts();
        assert_eq!(table.contract().to_string(), "eosio.token");
        assert_eq!(table.table_name().to_string(), "accounts");
        assert_eq!(table.scope().to_string(), "teamgreymass");
    }

    #[test]
    fn table_is_generic_over_row_type() {
        let table: Table<AccountRow> = Table::<AccountRow>::new(
            Name::new_from_str("eosio.token"),
            Name::new_from_str("accounts"),
            Name::new_from_str("teamgreymass"),
        );
        assert_eq!(table.contract().to_string(), "eosio.token");
    }

    #[tokio::test]
    #[ignore = "network; run manually"]
    async fn table_get_eosio_token_accounts_teamgreymass() {
        let client = jungle4_client();
        let table = teamgreymass_eosio_token_accounts();
        let row: Option<AccountRow> = table.get(5459781, &client).await.expect("table.get");
        assert!(row.is_some(), "EOS balance row should exist");
        let balance = row.unwrap().balance;
        println!("teamgreymass EOS balance on Jungle 4: {balance}");
        assert!(balance.amount() >= 0);
    }

    #[tokio::test]
    #[ignore = "network; run manually"]
    async fn table_all_returns_all_accounts_for_teamgreymass() {
        let client = jungle4_client();
        let table = teamgreymass_eosio_token_accounts();
        let rows: Vec<AccountRow> = table.all(&client).await.expect("table.all");
        assert!(
            !rows.is_empty(),
            "teamgreymass should have at least one balance row"
        );
        println!("Row count: {}", rows.len());
        for row in &rows {
            println!("  balance: {}", row.balance);
        }
    }
}
