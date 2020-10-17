package riko;

/** Represents a Rust error. */
public class ReturnedException extends RuntimeException {

  private final Error error;

  public ReturnedException(final Error src) {
    super(String.format("[Display] %1$s [Debug] %2$s", src.message, src.debug));
    this.error = src;
  }

  /**
   * Gets the debug info.
   *
   * <p>The {@code Debug} trait is used to generate this info.
   */
  public String getDebug() {
    return error.debug;
  }
}
