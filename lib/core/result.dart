import 'app_error.dart';

/// Sealed Result type for all fallible operations.
/// Exceptions are reserved for programmer errors (invariant violations).
sealed class Result<T> {
  const Result();

  factory Result.success(T value) = Success<T>;
  factory Result.failure(AppError error) = Failure<T>;

  bool get isSuccess => this is Success<T>;
  bool get isFailure => this is Failure<T>;

  T? get valueOrNull => switch (this) {
        Success<T>(:final value) => value,
        Failure<T>() => null,
      };

  AppError? get errorOrNull => switch (this) {
        Success<T>() => null,
        Failure<T>(:final error) => error,
      };

  Result<U> map<U>(U Function(T value) transform) => switch (this) {
        Success<T>(:final value) => Success(transform(value)),
        Failure<T>(:final error) => Failure(error),
      };

  Result<U> flatMap<U>(Result<U> Function(T value) transform) => switch (this) {
        Success<T>(:final value) => transform(value),
        Failure<T>(:final error) => Failure(error),
      };

  U fold<U>({
    required U Function(T value) onSuccess,
    required U Function(AppError error) onFailure,
  }) =>
      switch (this) {
        Success<T>(:final value) => onSuccess(value),
        Failure<T>(:final error) => onFailure(error),
      };
}

final class Success<T> extends Result<T> {
  const Success(this.value);
  final T value;

  @override
  bool operator ==(Object other) =>
      identical(this, other) || (other is Success<T> && other.value == value);

  @override
  int get hashCode => value.hashCode;

  @override
  String toString() => 'Success($value)';
}

final class Failure<T> extends Result<T> {
  const Failure(this.error);
  final AppError error;

  Failure<U> cast<U>() => Failure<U>(error);

  @override
  bool operator ==(Object other) =>
      identical(this, other) || (other is Failure<T> && other.error == error);

  @override
  int get hashCode => error.hashCode;

  @override
  String toString() => 'Failure($error)';
}
