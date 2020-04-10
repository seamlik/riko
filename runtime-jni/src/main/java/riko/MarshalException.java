package riko;

/** Error when marshaling data between FFI boundary. */
public class MarshalException extends RuntimeException {
  public MarshalException(final Exception cause) {
    super("Failed to marshal object", cause);
  }
}
