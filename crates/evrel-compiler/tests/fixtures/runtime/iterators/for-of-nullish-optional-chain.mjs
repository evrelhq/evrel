const visited = [];

for (const value of null?.items ?? ["fallback"]) {
    visited.push(value);
}

__evrel.observe("constant-nullish optional-chain iterator", visited);
