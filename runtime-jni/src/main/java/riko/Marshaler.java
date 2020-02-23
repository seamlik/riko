package riko;

import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.dataformat.cbor.CBORFactory;

/**
 * Marshals objects between the Rust side and the JNI side.
 */
public class Marshaler {
  private Marshaler() {}

  private static final ObjectMapper MAPPER = new ObjectMapper(new CBORFactory());

  /**
   * Serializes an object.
   */
  public static byte[] encode(final java.lang.Object src) {
    try {
      return MAPPER.writeValueAsBytes(src);
    } catch (final Exception err) {
      throw new RuntimeException("Failed to marshal object.", err);
    }
  }

  /**
   * Deserializes an object.
   */
  public static <T> Returned<T> decode(final byte[] src) {
    try {
      return MAPPER.readValue(src, new TypeReference<Returned<T>>() {});
    } catch (final Exception err) {
      throw new RuntimeException("Failed to marshal object.", err);
    }
  }
}
