package riko;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.dataformat.cbor.databind.CBORMapper;

/** Marshals objects between the Rust side and the JNI side. */
public class Marshaler {
  private Marshaler() {}

  public static final ObjectMapper MAPPER = new CBORMapper();

  /** Serializes an object. */
  public static byte[] encode(final java.lang.Object src) {
    try {
      return MAPPER.writeValueAsBytes(src);
    } catch (final Exception err) {
      throw new RuntimeException("Failed to marshal object.", err);
    }
  }

  /** Deserializes an object. */
  public static Returned decode(final byte[] src) {
    try {
      return MAPPER.readValue(src, Returned.class);
    } catch (final Exception err) {
      throw new RuntimeException("Failed to marshal object.", err);
    }
  }
}
