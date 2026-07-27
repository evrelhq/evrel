class Route {
  constructor(options) {
    if (options?.id && options?.path) {
      throw new Error("both");
    }

    this.result = options?.id || options?.path || "root";
  }
}

console.log(
  new Route({ id: "id" }).result,
  new Route({ path: "path" }).result,
  new Route(null).result,
);
