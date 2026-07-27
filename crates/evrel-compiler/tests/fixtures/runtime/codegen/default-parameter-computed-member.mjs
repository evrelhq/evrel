const Table = {
    Symbol: {
        Columns: "columns",
    },
};

class Query {
    constructor() {
        this.config = {
            table: {
                columns: 42,
            },
        };
    }

    returning(fields = this.config.table[Table.Symbol.Columns]) {
        return fields;
    }
}

__evrel.observe("default parameter computed member", new Query().returning());
