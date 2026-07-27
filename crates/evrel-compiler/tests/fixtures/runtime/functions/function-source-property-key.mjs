const object = {
    [() => {}]: 42,
};

__evrel.observe("function source property key", object[() => {}]);
