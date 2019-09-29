package riko;

public class UserException extends Exception {

  private final Error error;

  public UserException(final Error src) {
    super(src.message);
    this.error = src;
  }

  public String getDebug() {
    return error.debug;
  }
}
