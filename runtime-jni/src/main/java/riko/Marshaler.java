package riko;

import java.nio.ByteBuffer;
import org.bson.BsonBinaryReader;
import org.bson.BsonBinaryWriter;
import org.bson.BsonDocument;
import org.bson.BsonDocumentReader;
import org.bson.BsonValue;
import org.bson.codecs.BsonValueCodec;
import org.bson.codecs.DecoderContext;
import org.bson.codecs.configuration.CodecRegistries;
import org.bson.codecs.configuration.CodecRegistry;
import org.bson.codecs.pojo.PojoCodecProvider;
import org.bson.io.BasicOutputBuffer;
import org.checkerframework.checker.nullness.qual.Nullable;

/** Marshals objects between the Rust side and the JNI side. */
public class Marshaler {
  private Marshaler() {}

  private static final String ROOT_KEY_OF_ARGUMENT_DOCUMENT = "value";
  private static final CodecRegistry codecRegistry =
      CodecRegistries.fromRegistries(
          CodecRegistries.fromCodecs(new BsonValueCodec()),
          CodecRegistries.fromProviders(
              PojoCodecProvider.builder().register(Returned.class).build()));

  /** Serializes a function argument as BSON. */
  public static byte[] encode(final @Nullable BsonValue src) {
    if (src == null) {
      return new byte[0];
    }

    final BsonDocument document = new BsonDocument(ROOT_KEY_OF_ARGUMENT_DOCUMENT, src);
    try (final BasicOutputBuffer buffer = new BasicOutputBuffer();
        final BsonBinaryWriter writer = new BsonBinaryWriter(buffer);
        final BsonDocumentReader reader = new BsonDocumentReader(document)) {

      writer.pipe(reader);
      writer.flush();
      return buffer.toByteArray();
    }
  }

  /** Deserializes a BSON as the result from Rust side. */
  public static Returned decode(final byte[] src) {
    try (final BsonBinaryReader reader = new BsonBinaryReader(ByteBuffer.wrap(src))) {
      return codecRegistry.get(Returned.class).decode(reader, DecoderContext.builder().build());
    }
  }
}
