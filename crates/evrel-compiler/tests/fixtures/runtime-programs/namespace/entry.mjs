import * as namespace from "./values.mjs";
import { increment } from "./values.mjs";

__evrel.observe(
    "namespace before",
    namespace.value,
    namespace.default,
    Object.getPrototypeOf(namespace),
    Object.isSealed(namespace),
    Object.keys(namespace).join(","),
);
increment();
__evrel.observe("namespace after", namespace.value);
