package riko;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.dataformat.cbor.databind.CBORMapper;
import org.checkerframework.checker.nullness.qual.Nullable;

/** Marshals objects between the Rust side and the JNI side. */
public class Marshaler {
  private Marshaler() {}

  private static final ObjectMapper mapper = new CBORMapper();

  /** Serializes an object. */
  public static byte[] encode(final java.lang.@Nullable Object src) {
    try {
      return mapper.writeValueAsBytes(src);
    } catch (final Exception err) {
      throw new MarshalException(err);
    }
  }

  /** Deserializes an object. */
  public static Returned decode(final byte[] src) {
    try {
      return mapper.readValue(src, Returned.class);
    } catch (final Exception err) {
      throw new MarshalException(err);
    }
  }
}
