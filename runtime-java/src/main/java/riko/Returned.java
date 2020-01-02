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

  /**
   * Unwraps the returned value.
   * @throws ReturnedException If the Rust side returned an error.
   */
  @Nullable
  public T unwrap() throws ReturnedException {
    if (error != null) {
      throw new ReturnedException(error);
    } else {
      return value;
    }
  }
}
