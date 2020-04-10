package riko;

import com.fasterxml.jackson.databind.JsonNode;
import org.checkerframework.checker.nullness.qual.Nullable;

/** Data returned from the Rust side. */
public class Returned {
  @Nullable public Error error;

  @Nullable public JsonNode value;

  /**
   * Unwraps the returned value.
   *
   * <p>Jackson doesn't support building a {@link com.fasterxml.jackson.core.type.TypeReference}
   * with generics so a type must be provided.
   *
   * @throws ReturnedException If the Rust side returned an error.
   */
  @Nullable
  public <T> T unwrap(Class<T> type) {
    if (error != null) {
      throw new ReturnedException(error);
    } else {
      return Marshaler.MAPPER.convertValue(this.value, type);
    }
  }
}
