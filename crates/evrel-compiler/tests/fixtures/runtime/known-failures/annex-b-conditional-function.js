var before = typeof conditional;

if (true) function conditional() {
    return 42;
}

__evrel.observe("annex b conditional function", before, conditional());
