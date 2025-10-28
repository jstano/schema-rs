#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AggregationType {
    Sum,
    Count,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AggregationFrequency {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

#[derive(Debug, Clone)]
pub struct AggregationGroup {
    source: String,
    destination: String,
    source_derived_from: Option<String>,
}

impl AggregationGroup {
    pub fn new<S: Into<String>>(source: S, destination: S, source_derived_from: Option<S>) -> Self {
        Self {
            source: source.into(),
            destination: destination.into(),
            source_derived_from: source_derived_from.map(|s| s.into()),
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn destination(&self) -> &str {
        &self.destination
    }

    pub fn source_derived_from(&self) -> Option<&str> {
        self.source_derived_from.as_deref()
    }
}

#[derive(Debug, Clone)]
pub struct AggregationColumn {
    aggregation_type: AggregationType,
    source_column: String,
    destination_column: String,
}

impl AggregationColumn {
    pub fn new<S: Into<String>>(
        aggregation_type: AggregationType,
        source_column: S,
        destination_column: S,
    ) -> Self {
        Self {
            aggregation_type,
            source_column: source_column.into(),
            destination_column: destination_column.into(),
        }
    }

    pub fn aggregation_type(&self) -> AggregationType {
        self.aggregation_type
    }
    pub fn source_column(&self) -> &str {
        &self.source_column
    }
    pub fn destination_column(&self) -> &str {
        &self.destination_column
    }
}

#[derive(Debug, Clone)]
pub struct Aggregation {
    destination_table: String,
    date_column: String,
    criteria: Option<String>,
    time_stamp_column: String,
    aggregation_frequency: AggregationFrequency,
    aggregation_columns: Vec<AggregationColumn>,
    aggregation_groups: Vec<AggregationGroup>,
}

impl Aggregation {
    #[allow(clippy::too_many_arguments)]
    pub fn new<S: Into<String>>(
        destination_table: S,
        date_column: S,
        criteria: Option<S>,
        time_stamp_column: S,
        aggregation_frequency: AggregationFrequency,
        aggregation_columns: Vec<AggregationColumn>,
        aggregation_groups: Vec<AggregationGroup>,
    ) -> Self {
        Self {
            destination_table: destination_table.into(),
            date_column: date_column.into(),
            criteria: criteria.map(|s| s.into()),
            time_stamp_column: time_stamp_column.into(),
            aggregation_frequency,
            aggregation_columns,
            aggregation_groups,
        }
    }

    pub fn destination_table(&self) -> &str {
        &self.destination_table
    }

    pub fn date_column(&self) -> &str {
        &self.date_column
    }

    pub fn criteria(&self) -> Option<&str> {
        self.criteria.as_deref()
    }

    pub fn time_stamp_column(&self) -> &str {
        &self.time_stamp_column
    }

    pub fn aggregation_frequency(&self) -> AggregationFrequency {
        self.aggregation_frequency
    }

    pub fn aggregation_groups(&self) -> &Vec<AggregationGroup> {
        &self.aggregation_groups
    }

    pub fn aggregation_columns(&self) -> &Vec<AggregationColumn> {
        &self.aggregation_columns
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregation_group_new_and_getters() {
        let g = AggregationGroup::new("src", "dest", Some("derived"));
        assert_eq!(g.source(), "src");
        assert_eq!(g.destination(), "dest");
        assert_eq!(g.source_derived_from().unwrap(), "derived");
    }

    #[test]
    fn aggregation_column_new_and_getters() {
        let c = AggregationColumn::new(AggregationType::Sum, "s_col", "d_col");
        assert_eq!(c.aggregation_type(), AggregationType::Sum);
        assert_eq!(c.source_column(), "s_col");
        assert_eq!(c.destination_column(), "d_col");
    }

    #[test]
    fn aggregation_new_and_getters() {
        let cols = vec![
            AggregationColumn::new(AggregationType::Sum, "a", "a_sum"),
            AggregationColumn::new(AggregationType::Count, "b", "b_cnt"),
        ];
        let groups = vec![
            AggregationGroup::new("src1", "dst1", Some("from1")),
            AggregationGroup::new("src2", "dst2", Some("from2")),
        ];
        let aggr = Aggregation::new(
            "dest_table",
            "date_col",
            Some("criteria"),
            "ts_col",
            AggregationFrequency::Monthly,
            cols.clone(),
            groups.clone(),
        );

        assert_eq!(aggr.destination_table(), "dest_table");
        assert_eq!(aggr.date_column(), "date_col");
        assert_eq!(aggr.criteria().unwrap(), "criteria");
        assert_eq!(aggr.time_stamp_column(), "ts_col");
        assert_eq!(aggr.aggregation_frequency(), AggregationFrequency::Monthly);
        assert_eq!(aggr.aggregation_columns().len(), 2);
        assert_eq!(aggr.aggregation_groups().len(), 2);

        // Spot-check preserved content
        assert_eq!(aggr.aggregation_columns()[0].source_column(), "a");
        assert_eq!(
            aggr.aggregation_columns()[1].aggregation_type(),
            AggregationType::Count
        );
        assert_eq!(aggr.aggregation_groups()[0].destination(), "dst1");
        assert_eq!(aggr.aggregation_groups()[1].source_derived_from().unwrap(), "from2");

        // Ensure vectors returned are the same size as provided (by value semantics they were moved in)
        assert_eq!(aggr.aggregation_columns().len(), cols.len());
        assert_eq!(aggr.aggregation_groups().len(), groups.len());
    }
}
