var outer = 1;
function run() {
    var local = 2;
    var result = eval("local + outer");
    eval("local = 40");
    return [result, local];
}

var values = run();
__evrel.observe("direct eval", values[0], values[1]);
