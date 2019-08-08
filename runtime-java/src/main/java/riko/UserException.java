package riko;

public class UserException extends Exception {

  public UserException(final Error src) {
    super(src.message);
  }
}
