package riko;

import org.bson.BsonValue;
import org.checkerframework.checker.nullness.qual.Nullable;

/** Data returned from the Rust side. */
public class Returned {
  public @Nullable Error error;

  public @Nullable BsonValue value;

  /**
   * Unwraps the returned value.
   *
   * @throws ReturnedException If the Rust side returned an error.
   */
  public @Nullable BsonValue unwrap() {
    if (error != null) {
      throw new ReturnedException(error);
    } else if (value == null || value.isNull()) {
      return null;
    } else {
      return value;
    }
  }
}
