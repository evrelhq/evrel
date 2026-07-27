const source = "export const value = 42; export default 'default';";
const encoded = Buffer.from(source).toString("base64");
const namespace = await import(`data:text/javascript;base64,${encoded}`);

__evrel.observe(
    "dynamic import",
    namespace.value,
    namespace.default,
    Object.isSealed(namespace),
    Object.getPrototypeOf(namespace),
);
