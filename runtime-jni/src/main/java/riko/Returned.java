package riko;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.checkerframework.checker.nullness.qual.Nullable;

/** Data returned from the Rust side. */
public class Returned {
  private static final ObjectMapper mapper = new ObjectMapper();

  public @Nullable Error error;

  public @Nullable JsonNode value;

  /**
   * Unwraps the returned value.
   *
   * <p>Jackson doesn't support building a {@link com.fasterxml.jackson.core.type.TypeReference}
   * with generics so a type must be provided.
   *
   * @throws ReturnedException If the Rust side returned an error.
   */
  public @Nullable <T> T unwrap(Class<T> type) {
    if (error != null) {
      throw new ReturnedException(error);
    } else if (value != null) {
      return mapper.convertValue(this.value, type);
    } else {
      return null;
    }
  }
}
