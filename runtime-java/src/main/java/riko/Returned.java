package riko;

import org.checkerframework.checker.nullness.qual.Nullable;

/**
 * Data returned from the Rust side.
 */
public class Returned<T> {
  @Nullable
  public Error error;

  @Nullable
  public T value;

  @Nullable
  public T unwrap() throws UserException {
    if (error != null) {
      throw new UserException(error);
    } else {
      return value;
    }
  }
}
