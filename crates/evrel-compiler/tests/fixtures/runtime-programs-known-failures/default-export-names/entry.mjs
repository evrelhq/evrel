import FunctionDefault from "./function.mjs";
import ClassDefault from "./class.mjs";

__evrel.observe(
    "default export names",
    FunctionDefault.name,
    FunctionDefault(),
    ClassDefault.name,
    new ClassDefault().value,
);
