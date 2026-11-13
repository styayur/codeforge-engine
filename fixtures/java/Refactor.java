public final class Refactor {
    public static void run() {
        try {
            Integer.parseInt("not-a-number");
        } catch (NumberFormatException ignored) {}
    }
}
