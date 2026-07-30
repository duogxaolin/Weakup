import 'package:flutter_test/flutter_test.dart';
import 'package:weakup/core/core.dart';

void main() {
  group('Result', () {
    group('Success', () {
      test('construction', () {
        final r = Result<int>.success(42);
        expect(r, isA<Success<int>>());
        expect((r as Success<int>).value, 42);
      });

      test('isSuccess is true', () {
        expect(Result.success(1).isSuccess, isTrue);
      });

      test('isFailure is false', () {
        expect(Result.success(1).isFailure, isFalse);
      });

      test('valueOrNull returns value', () {
        expect(Result.success('hello').valueOrNull, 'hello');
      });

      test('errorOrNull returns null', () {
        expect(Result<String>.success('x').errorOrNull, isNull);
      });

      test('map transforms value', () {
        final r = Result.success(2).map((v) => v * 10);
        expect(r, isA<Success<int>>());
        expect((r as Success<int>).value, 20);
      });

      test('flatMap to success', () {
        final r = Result.success(5).flatMap((v) => Result.success(v + 1));
        expect((r as Success<int>).value, 6);
      });

      test('flatMap to failure', () {
        const err = ValidationError(message: 'bad');
        final r = Result<int>.success(5).flatMap<int>((v) => Result.failure(err));
        expect(r, isA<Failure<int>>());
      });

      test('fold calls onSuccess', () {
        final v = Result.success(7).fold(
          onSuccess: (val) => 'ok:$val',
          onFailure: (_) => 'fail',
        );
        expect(v, 'ok:7');
      });

      test('equality', () {
        expect(Result.success(3), equals(Result.success(3)));
        expect(Result.success(3), isNot(equals(Result.success(4))));
      });
    });

    group('Failure', () {
      const err = PowerOffUnsupported();

      test('construction', () {
        final r = Result<void>.failure(err);
        expect(r, isA<Failure<void>>());
      });

      test('isSuccess is false', () {
        expect(Result<void>.failure(err).isSuccess, isFalse);
      });

      test('isFailure is true', () {
        expect(Result<void>.failure(err).isFailure, isTrue);
      });

      test('valueOrNull returns null', () {
        expect(Result<int>.failure(err).valueOrNull, isNull);
      });

      test('errorOrNull returns error', () {
        expect(Result<int>.failure(err).errorOrNull, err);
      });

      test('map preserves failure', () {
        final r = Result<int>.failure(err).map((v) => v * 2);
        expect(r, isA<Failure<int>>());
        expect((r as Failure<int>).error, err);
      });

      test('flatMap preserves failure', () {
        final r = Result<int>.failure(err).flatMap((v) => Result.success(v));
        expect(r, isA<Failure<int>>());
      });

      test('fold calls onFailure', () {
        final v = Result<int>.failure(err).fold(
          onSuccess: (_) => 'ok',
          onFailure: (e) => 'fail:$e',
        );
        expect(v, startsWith('fail:'));
      });

      test('equality', () {
        final e = const StorageError(message: 'disk full');
        expect(Result<void>.failure(e), equals(Result<void>.failure(e)));
        expect(
          Result<void>.failure(e),
          isNot(equals(Result<void>.failure(const StorageError(message: 'other')))),
        );
      });

      test('cast preserves error', () {
        final f = Failure<int>(err);
        final casted = f.cast<String>();
        expect(casted.error, err);
      });
    });
  });
}
