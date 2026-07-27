const topLevelThis = this;
const meta = import.meta;

__evrel.observe(
    "module basics",
    topLevelThis,
    typeof meta,
    typeof meta.url,
    meta.url.startsWith("data:text/javascript"),
);
