var before = typeof labelled;
label: function labelled() {
    return 42;
}

__evrel.observe("annex b labelled function", before, labelled());
