import { namespace } from "./bridge.mjs";

__evrel.observe(
    "export namespace",
    namespace.value,
    namespace.default,
    Object.getPrototypeOf(namespace),
);
