package riko;

/** Represents a Rust error. */
public class ReturnedException extends RuntimeException {

  private final Error error;

  public ReturnedException(final Error src) {
    super(src.message);
    this.error = src;
  }

  /** Gets the debug info. The {@code Debug} trait is used to generate this info. */
  public String getDebug() {
    return error.debug;
  }
}
