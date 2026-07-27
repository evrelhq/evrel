function hash(state, previous, value) {
    return previous + value;
}

function update(state, index, minimumMatch) {
    state.hash = hash(
        state,
        state.hash,
        state.window[index + minimumMatch - 1],
    );
}

const state = {
    hash: 2,
    window: [10, 20, 30, 40],
};

update(state, 1, 3);
__evrel.observe("nested computed call argument", state.hash);
