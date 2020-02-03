import java.nio.file.Paths;
import org.junit.jupiter.api.Assertions;
import org.junit.jupiter.api.Test;
import riko_sample.Module;

class Tests {
  static {
    final String library = Paths
        .get("..", "target", "debug", "libriko_sample.so")
        .toAbsolutePath()
        .toString();
    System.load(library);
  }

  @Test
  void nothing() {
    Module.nothing();
  }

  @Test
  void i32() {
    Assertions.assertEquals(2, Module._i32(1, 1));
  }

  @Test
  void rename() {
    Module.rename();
  }

  @Test
  void result_option() {
    Assertions.assertEquals(2, Module.result_option(1, 1));
    Assertions.assertThrows(Exception.class, () -> Module.result_option(null, null));
    Assertions.assertNull(Module.result_option(null, 1));
  }

  @Test
  void marshal() {
    Assertions.assertEquals(-1, Module.marshal(1));
  }

  @Test
  void string() {
    Assertions.assertEquals("love you", Module.string("love", " you"));
  }

  @Test
  void bytes() {
    byte[] a = { 1, 2, 3 };
    byte[] b = { 4, 5, 6 };
    byte[] expected = { 1, 2, 3, 4, 5, 6 };
    Assertions.assertArrayEquals(expected, Module.bytes(a, b));
  }

  @Test
  void bool() {
    Assertions.assertEquals(false, Module._bool(false, true));
  }
}