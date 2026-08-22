// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'app_database.dart';

// ignore_for_file: type=lint
class $JobsTable extends Jobs with TableInfo<$JobsTable, JobRow> {
  @override
  final GeneratedDatabase attachedDatabase;
  final String? _alias;
  $JobsTable(this.attachedDatabase, [this._alias]);
  static const VerificationMeta _idMeta = const VerificationMeta('id');
  @override
  late final GeneratedColumn<int> id = GeneratedColumn<int>(
    'id',
    aliasedName,
    false,
    hasAutoIncrement: true,
    type: DriftSqlType.int,
    requiredDuringInsert: false,
    defaultConstraints: GeneratedColumn.constraintIsAlways(
      'PRIMARY KEY AUTOINCREMENT',
    ),
  );
  static const VerificationMeta _typeMeta = const VerificationMeta('type');
  @override
  late final GeneratedColumn<String> type = GeneratedColumn<String>(
    'type',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _triggerKindMeta = const VerificationMeta(
    'triggerKind',
  );
  @override
  late final GeneratedColumn<String> triggerKind = GeneratedColumn<String>(
    'trigger_kind',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _triggerMinutesMeta = const VerificationMeta(
    'triggerMinutes',
  );
  @override
  late final GeneratedColumn<int> triggerMinutes = GeneratedColumn<int>(
    'trigger_minutes',
    aliasedName,
    true,
    type: DriftSqlType.int,
    requiredDuringInsert: false,
  );
  static const VerificationMeta _triggerHourMeta = const VerificationMeta(
    'triggerHour',
  );
  @override
  late final GeneratedColumn<int> triggerHour = GeneratedColumn<int>(
    'trigger_hour',
    aliasedName,
    true,
    type: DriftSqlType.int,
    requiredDuringInsert: false,
  );
  static const VerificationMeta _triggerMinuteMeta = const VerificationMeta(
    'triggerMinute',
  );
  @override
  late final GeneratedColumn<int> triggerMinute = GeneratedColumn<int>(
    'trigger_minute',
    aliasedName,
    true,
    type: DriftSqlType.int,
    requiredDuringInsert: false,
  );
  static const VerificationMeta _triggerDateMeta = const VerificationMeta(
    'triggerDate',
  );
  @override
  late final GeneratedColumn<String> triggerDate = GeneratedColumn<String>(
    'trigger_date',
    aliasedName,
    true,
    type: DriftSqlType.string,
    requiredDuringInsert: false,
  );
  static const VerificationMeta _statusMeta = const VerificationMeta('status');
  @override
  late final GeneratedColumn<String> status = GeneratedColumn<String>(
    'status',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _targetInstantUtcMeta = const VerificationMeta(
    'targetInstantUtc',
  );
  @override
  late final GeneratedColumn<DateTime> targetInstantUtc =
      GeneratedColumn<DateTime>(
        'target_instant_utc',
        aliasedName,
        true,
        type: DriftSqlType.dateTime,
        requiredDuringInsert: false,
      );
  static const VerificationMeta _createdAtMeta = const VerificationMeta(
    'createdAt',
  );
  @override
  late final GeneratedColumn<DateTime> createdAt = GeneratedColumn<DateTime>(
    'created_at',
    aliasedName,
    false,
    type: DriftSqlType.dateTime,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _updatedAtMeta = const VerificationMeta(
    'updatedAt',
  );
  @override
  late final GeneratedColumn<DateTime> updatedAt = GeneratedColumn<DateTime>(
    'updated_at',
    aliasedName,
    false,
    type: DriftSqlType.dateTime,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _failureMessageMeta = const VerificationMeta(
    'failureMessage',
  );
  @override
  late final GeneratedColumn<String> failureMessage = GeneratedColumn<String>(
    'failure_message',
    aliasedName,
    true,
    type: DriftSqlType.string,
    requiredDuringInsert: false,
  );
  static const VerificationMeta _originMeta = const VerificationMeta('origin');
  @override
  late final GeneratedColumn<String> origin = GeneratedColumn<String>(
    'origin',
    aliasedName,
    true,
    type: DriftSqlType.string,
    requiredDuringInsert: false,
  );
  @override
  List<GeneratedColumn> get $columns => [
    id,
    type,
    triggerKind,
    triggerMinutes,
    triggerHour,
    triggerMinute,
    triggerDate,
    status,
    targetInstantUtc,
    createdAt,
    updatedAt,
    failureMessage,
    origin,
  ];
  @override
  String get aliasedName => _alias ?? actualTableName;
  @override
  String get actualTableName => $name;
  static const String $name = 'jobs';
  @override
  VerificationContext validateIntegrity(
    Insertable<JobRow> instance, {
    bool isInserting = false,
  }) {
    final context = VerificationContext();
    final data = instance.toColumns(true);
    if (data.containsKey('id')) {
      context.handle(_idMeta, id.isAcceptableOrUnknown(data['id']!, _idMeta));
    }
    if (data.containsKey('type')) {
      context.handle(
        _typeMeta,
        type.isAcceptableOrUnknown(data['type']!, _typeMeta),
      );
    } else if (isInserting) {
      context.missing(_typeMeta);
    }
    if (data.containsKey('trigger_kind')) {
      context.handle(
        _triggerKindMeta,
        triggerKind.isAcceptableOrUnknown(
          data['trigger_kind']!,
          _triggerKindMeta,
        ),
      );
    } else if (isInserting) {
      context.missing(_triggerKindMeta);
    }
    if (data.containsKey('trigger_minutes')) {
      context.handle(
        _triggerMinutesMeta,
        triggerMinutes.isAcceptableOrUnknown(
          data['trigger_minutes']!,
          _triggerMinutesMeta,
        ),
      );
    }
    if (data.containsKey('trigger_hour')) {
      context.handle(
        _triggerHourMeta,
        triggerHour.isAcceptableOrUnknown(
          data['trigger_hour']!,
          _triggerHourMeta,
        ),
      );
    }
    if (data.containsKey('trigger_minute')) {
      context.handle(
        _triggerMinuteMeta,
        triggerMinute.isAcceptableOrUnknown(
          data['trigger_minute']!,
          _triggerMinuteMeta,
        ),
      );
    }
    if (data.containsKey('trigger_date')) {
      context.handle(
        _triggerDateMeta,
        triggerDate.isAcceptableOrUnknown(
          data['trigger_date']!,
          _triggerDateMeta,
        ),
      );
    }
    if (data.containsKey('status')) {
      context.handle(
        _statusMeta,
        status.isAcceptableOrUnknown(data['status']!, _statusMeta),
      );
    } else if (isInserting) {
      context.missing(_statusMeta);
    }
    if (data.containsKey('target_instant_utc')) {
      context.handle(
        _targetInstantUtcMeta,
        targetInstantUtc.isAcceptableOrUnknown(
          data['target_instant_utc']!,
          _targetInstantUtcMeta,
        ),
      );
    }
    if (data.containsKey('created_at')) {
      context.handle(
        _createdAtMeta,
        createdAt.isAcceptableOrUnknown(data['created_at']!, _createdAtMeta),
      );
    } else if (isInserting) {
      context.missing(_createdAtMeta);
    }
    if (data.containsKey('updated_at')) {
      context.handle(
        _updatedAtMeta,
        updatedAt.isAcceptableOrUnknown(data['updated_at']!, _updatedAtMeta),
      );
    } else if (isInserting) {
      context.missing(_updatedAtMeta);
    }
    if (data.containsKey('failure_message')) {
      context.handle(
        _failureMessageMeta,
        failureMessage.isAcceptableOrUnknown(
          data['failure_message']!,
          _failureMessageMeta,
        ),
      );
    }
    if (data.containsKey('origin')) {
      context.handle(
        _originMeta,
        origin.isAcceptableOrUnknown(data['origin']!, _originMeta),
      );
    }
    return context;
  }

  @override
  Set<GeneratedColumn> get $primaryKey => {id};
  @override
  JobRow map(Map<String, dynamic> data, {String? tablePrefix}) {
    final effectivePrefix = tablePrefix != null ? '$tablePrefix.' : '';
    return JobRow(
      id: attachedDatabase.typeMapping.read(
        DriftSqlType.int,
        data['${effectivePrefix}id'],
      )!,
      type: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}type'],
      )!,
      triggerKind: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}trigger_kind'],
      )!,
      triggerMinutes: attachedDatabase.typeMapping.read(
        DriftSqlType.int,
        data['${effectivePrefix}trigger_minutes'],
      ),
      triggerHour: attachedDatabase.typeMapping.read(
        DriftSqlType.int,
        data['${effectivePrefix}trigger_hour'],
      ),
      triggerMinute: attachedDatabase.typeMapping.read(
        DriftSqlType.int,
        data['${effectivePrefix}trigger_minute'],
      ),
      triggerDate: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}trigger_date'],
      ),
      status: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}status'],
      )!,
      targetInstantUtc: attachedDatabase.typeMapping.read(
        DriftSqlType.dateTime,
        data['${effectivePrefix}target_instant_utc'],
      ),
      createdAt: attachedDatabase.typeMapping.read(
        DriftSqlType.dateTime,
        data['${effectivePrefix}created_at'],
      )!,
      updatedAt: attachedDatabase.typeMapping.read(
        DriftSqlType.dateTime,
        data['${effectivePrefix}updated_at'],
      )!,
      failureMessage: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}failure_message'],
      ),
      origin: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}origin'],
      ),
    );
  }

  @override
  $JobsTable createAlias(String alias) {
    return $JobsTable(attachedDatabase, alias);
  }
}

class JobRow extends DataClass implements Insertable<JobRow> {
  final int id;
  final String type;
  final String triggerKind;
  final int? triggerMinutes;
  final int? triggerHour;
  final int? triggerMinute;

  /// The exact local date an AbsoluteTimeTrigger fires on, as `YYYY-MM-DD`.
  ///
  /// Nullable, and null means "a time of day" — which is what every row written
  /// before this column existed meant, so the migration needs no backfill.
  ///
  /// Text rather than a `DateTimeColumn`: a wall-clock date is not an instant, and
  /// storing it as one would attach a zone the value must not carry.
  final String? triggerDate;
  final String status;
  final DateTime? targetInstantUtc;
  final DateTime createdAt;
  final DateTime updatedAt;
  final String? failureMessage;

  /// Whether this job was scheduled at this device or created by an authorized remote
  /// command, as `local` or `remote`.
  ///
  /// Nullable, and null means `local` — which is what every row written before this column
  /// existed meant, so the migration needs no backfill.
  ///
  /// Not cosmetic: the countdown a power-off job receives is chosen from its origin, so an
  /// origin that did not survive a restart handed a remote-initiated shutdown the shorter
  /// local countdown.
  final String? origin;
  const JobRow({
    required this.id,
    required this.type,
    required this.triggerKind,
    this.triggerMinutes,
    this.triggerHour,
    this.triggerMinute,
    this.triggerDate,
    required this.status,
    this.targetInstantUtc,
    required this.createdAt,
    required this.updatedAt,
    this.failureMessage,
    this.origin,
  });
  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    map['id'] = Variable<int>(id);
    map['type'] = Variable<String>(type);
    map['trigger_kind'] = Variable<String>(triggerKind);
    if (!nullToAbsent || triggerMinutes != null) {
      map['trigger_minutes'] = Variable<int>(triggerMinutes);
    }
    if (!nullToAbsent || triggerHour != null) {
      map['trigger_hour'] = Variable<int>(triggerHour);
    }
    if (!nullToAbsent || triggerMinute != null) {
      map['trigger_minute'] = Variable<int>(triggerMinute);
    }
    if (!nullToAbsent || triggerDate != null) {
      map['trigger_date'] = Variable<String>(triggerDate);
    }
    map['status'] = Variable<String>(status);
    if (!nullToAbsent || targetInstantUtc != null) {
      map['target_instant_utc'] = Variable<DateTime>(targetInstantUtc);
    }
    map['created_at'] = Variable<DateTime>(createdAt);
    map['updated_at'] = Variable<DateTime>(updatedAt);
    if (!nullToAbsent || failureMessage != null) {
      map['failure_message'] = Variable<String>(failureMessage);
    }
    if (!nullToAbsent || origin != null) {
      map['origin'] = Variable<String>(origin);
    }
    return map;
  }

  JobsCompanion toCompanion(bool nullToAbsent) {
    return JobsCompanion(
      id: Value(id),
      type: Value(type),
      triggerKind: Value(triggerKind),
      triggerMinutes: triggerMinutes == null && nullToAbsent
          ? const Value.absent()
          : Value(triggerMinutes),
      triggerHour: triggerHour == null && nullToAbsent
          ? const Value.absent()
          : Value(triggerHour),
      triggerMinute: triggerMinute == null && nullToAbsent
          ? const Value.absent()
          : Value(triggerMinute),
      triggerDate: triggerDate == null && nullToAbsent
          ? const Value.absent()
          : Value(triggerDate),
      status: Value(status),
      targetInstantUtc: targetInstantUtc == null && nullToAbsent
          ? const Value.absent()
          : Value(targetInstantUtc),
      createdAt: Value(createdAt),
      updatedAt: Value(updatedAt),
      failureMessage: failureMessage == null && nullToAbsent
          ? const Value.absent()
          : Value(failureMessage),
      origin: origin == null && nullToAbsent
          ? const Value.absent()
          : Value(origin),
    );
  }

  factory JobRow.fromJson(
    Map<String, dynamic> json, {
    ValueSerializer? serializer,
  }) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return JobRow(
      id: serializer.fromJson<int>(json['id']),
      type: serializer.fromJson<String>(json['type']),
      triggerKind: serializer.fromJson<String>(json['triggerKind']),
      triggerMinutes: serializer.fromJson<int?>(json['triggerMinutes']),
      triggerHour: serializer.fromJson<int?>(json['triggerHour']),
      triggerMinute: serializer.fromJson<int?>(json['triggerMinute']),
      triggerDate: serializer.fromJson<String?>(json['triggerDate']),
      status: serializer.fromJson<String>(json['status']),
      targetInstantUtc: serializer.fromJson<DateTime?>(
        json['targetInstantUtc'],
      ),
      createdAt: serializer.fromJson<DateTime>(json['createdAt']),
      updatedAt: serializer.fromJson<DateTime>(json['updatedAt']),
      failureMessage: serializer.fromJson<String?>(json['failureMessage']),
      origin: serializer.fromJson<String?>(json['origin']),
    );
  }
  @override
  Map<String, dynamic> toJson({ValueSerializer? serializer}) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return <String, dynamic>{
      'id': serializer.toJson<int>(id),
      'type': serializer.toJson<String>(type),
      'triggerKind': serializer.toJson<String>(triggerKind),
      'triggerMinutes': serializer.toJson<int?>(triggerMinutes),
      'triggerHour': serializer.toJson<int?>(triggerHour),
      'triggerMinute': serializer.toJson<int?>(triggerMinute),
      'triggerDate': serializer.toJson<String?>(triggerDate),
      'status': serializer.toJson<String>(status),
      'targetInstantUtc': serializer.toJson<DateTime?>(targetInstantUtc),
      'createdAt': serializer.toJson<DateTime>(createdAt),
      'updatedAt': serializer.toJson<DateTime>(updatedAt),
      'failureMessage': serializer.toJson<String?>(failureMessage),
      'origin': serializer.toJson<String?>(origin),
    };
  }

  JobRow copyWith({
    int? id,
    String? type,
    String? triggerKind,
    Value<int?> triggerMinutes = const Value.absent(),
    Value<int?> triggerHour = const Value.absent(),
    Value<int?> triggerMinute = const Value.absent(),
    Value<String?> triggerDate = const Value.absent(),
    String? status,
    Value<DateTime?> targetInstantUtc = const Value.absent(),
    DateTime? createdAt,
    DateTime? updatedAt,
    Value<String?> failureMessage = const Value.absent(),
    Value<String?> origin = const Value.absent(),
  }) => JobRow(
    id: id ?? this.id,
    type: type ?? this.type,
    triggerKind: triggerKind ?? this.triggerKind,
    triggerMinutes: triggerMinutes.present
        ? triggerMinutes.value
        : this.triggerMinutes,
    triggerHour: triggerHour.present ? triggerHour.value : this.triggerHour,
    triggerMinute: triggerMinute.present
        ? triggerMinute.value
        : this.triggerMinute,
    triggerDate: triggerDate.present ? triggerDate.value : this.triggerDate,
    status: status ?? this.status,
    targetInstantUtc: targetInstantUtc.present
        ? targetInstantUtc.value
        : this.targetInstantUtc,
    createdAt: createdAt ?? this.createdAt,
    updatedAt: updatedAt ?? this.updatedAt,
    failureMessage: failureMessage.present
        ? failureMessage.value
        : this.failureMessage,
    origin: origin.present ? origin.value : this.origin,
  );
  JobRow copyWithCompanion(JobsCompanion data) {
    return JobRow(
      id: data.id.present ? data.id.value : this.id,
      type: data.type.present ? data.type.value : this.type,
      triggerKind: data.triggerKind.present
          ? data.triggerKind.value
          : this.triggerKind,
      triggerMinutes: data.triggerMinutes.present
          ? data.triggerMinutes.value
          : this.triggerMinutes,
      triggerHour: data.triggerHour.present
          ? data.triggerHour.value
          : this.triggerHour,
      triggerMinute: data.triggerMinute.present
          ? data.triggerMinute.value
          : this.triggerMinute,
      triggerDate: data.triggerDate.present
          ? data.triggerDate.value
          : this.triggerDate,
      status: data.status.present ? data.status.value : this.status,
      targetInstantUtc: data.targetInstantUtc.present
          ? data.targetInstantUtc.value
          : this.targetInstantUtc,
      createdAt: data.createdAt.present ? data.createdAt.value : this.createdAt,
      updatedAt: data.updatedAt.present ? data.updatedAt.value : this.updatedAt,
      failureMessage: data.failureMessage.present
          ? data.failureMessage.value
          : this.failureMessage,
      origin: data.origin.present ? data.origin.value : this.origin,
    );
  }

  @override
  String toString() {
    return (StringBuffer('JobRow(')
          ..write('id: $id, ')
          ..write('type: $type, ')
          ..write('triggerKind: $triggerKind, ')
          ..write('triggerMinutes: $triggerMinutes, ')
          ..write('triggerHour: $triggerHour, ')
          ..write('triggerMinute: $triggerMinute, ')
          ..write('triggerDate: $triggerDate, ')
          ..write('status: $status, ')
          ..write('targetInstantUtc: $targetInstantUtc, ')
          ..write('createdAt: $createdAt, ')
          ..write('updatedAt: $updatedAt, ')
          ..write('failureMessage: $failureMessage, ')
          ..write('origin: $origin')
          ..write(')'))
        .toString();
  }

  @override
  int get hashCode => Object.hash(
    id,
    type,
    triggerKind,
    triggerMinutes,
    triggerHour,
    triggerMinute,
    triggerDate,
    status,
    targetInstantUtc,
    createdAt,
    updatedAt,
    failureMessage,
    origin,
  );
  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is JobRow &&
          other.id == this.id &&
          other.type == this.type &&
          other.triggerKind == this.triggerKind &&
          other.triggerMinutes == this.triggerMinutes &&
          other.triggerHour == this.triggerHour &&
          other.triggerMinute == this.triggerMinute &&
          other.triggerDate == this.triggerDate &&
          other.status == this.status &&
          other.targetInstantUtc == this.targetInstantUtc &&
          other.createdAt == this.createdAt &&
          other.updatedAt == this.updatedAt &&
          other.failureMessage == this.failureMessage &&
          other.origin == this.origin);
}

class JobsCompanion extends UpdateCompanion<JobRow> {
  final Value<int> id;
  final Value<String> type;
  final Value<String> triggerKind;
  final Value<int?> triggerMinutes;
  final Value<int?> triggerHour;
  final Value<int?> triggerMinute;
  final Value<String?> triggerDate;
  final Value<String> status;
  final Value<DateTime?> targetInstantUtc;
  final Value<DateTime> createdAt;
  final Value<DateTime> updatedAt;
  final Value<String?> failureMessage;
  final Value<String?> origin;
  const JobsCompanion({
    this.id = const Value.absent(),
    this.type = const Value.absent(),
    this.triggerKind = const Value.absent(),
    this.triggerMinutes = const Value.absent(),
    this.triggerHour = const Value.absent(),
    this.triggerMinute = const Value.absent(),
    this.triggerDate = const Value.absent(),
    this.status = const Value.absent(),
    this.targetInstantUtc = const Value.absent(),
    this.createdAt = const Value.absent(),
    this.updatedAt = const Value.absent(),
    this.failureMessage = const Value.absent(),
    this.origin = const Value.absent(),
  });
  JobsCompanion.insert({
    this.id = const Value.absent(),
    required String type,
    required String triggerKind,
    this.triggerMinutes = const Value.absent(),
    this.triggerHour = const Value.absent(),
    this.triggerMinute = const Value.absent(),
    this.triggerDate = const Value.absent(),
    required String status,
    this.targetInstantUtc = const Value.absent(),
    required DateTime createdAt,
    required DateTime updatedAt,
    this.failureMessage = const Value.absent(),
    this.origin = const Value.absent(),
  }) : type = Value(type),
       triggerKind = Value(triggerKind),
       status = Value(status),
       createdAt = Value(createdAt),
       updatedAt = Value(updatedAt);
  static Insertable<JobRow> custom({
    Expression<int>? id,
    Expression<String>? type,
    Expression<String>? triggerKind,
    Expression<int>? triggerMinutes,
    Expression<int>? triggerHour,
    Expression<int>? triggerMinute,
    Expression<String>? triggerDate,
    Expression<String>? status,
    Expression<DateTime>? targetInstantUtc,
    Expression<DateTime>? createdAt,
    Expression<DateTime>? updatedAt,
    Expression<String>? failureMessage,
    Expression<String>? origin,
  }) {
    return RawValuesInsertable({
      if (id != null) 'id': id,
      if (type != null) 'type': type,
      if (triggerKind != null) 'trigger_kind': triggerKind,
      if (triggerMinutes != null) 'trigger_minutes': triggerMinutes,
      if (triggerHour != null) 'trigger_hour': triggerHour,
      if (triggerMinute != null) 'trigger_minute': triggerMinute,
      if (triggerDate != null) 'trigger_date': triggerDate,
      if (status != null) 'status': status,
      if (targetInstantUtc != null) 'target_instant_utc': targetInstantUtc,
      if (createdAt != null) 'created_at': createdAt,
      if (updatedAt != null) 'updated_at': updatedAt,
      if (failureMessage != null) 'failure_message': failureMessage,
      if (origin != null) 'origin': origin,
    });
  }

  JobsCompanion copyWith({
    Value<int>? id,
    Value<String>? type,
    Value<String>? triggerKind,
    Value<int?>? triggerMinutes,
    Value<int?>? triggerHour,
    Value<int?>? triggerMinute,
    Value<String?>? triggerDate,
    Value<String>? status,
    Value<DateTime?>? targetInstantUtc,
    Value<DateTime>? createdAt,
    Value<DateTime>? updatedAt,
    Value<String?>? failureMessage,
    Value<String?>? origin,
  }) {
    return JobsCompanion(
      id: id ?? this.id,
      type: type ?? this.type,
      triggerKind: triggerKind ?? this.triggerKind,
      triggerMinutes: triggerMinutes ?? this.triggerMinutes,
      triggerHour: triggerHour ?? this.triggerHour,
      triggerMinute: triggerMinute ?? this.triggerMinute,
      triggerDate: triggerDate ?? this.triggerDate,
      status: status ?? this.status,
      targetInstantUtc: targetInstantUtc ?? this.targetInstantUtc,
      createdAt: createdAt ?? this.createdAt,
      updatedAt: updatedAt ?? this.updatedAt,
      failureMessage: failureMessage ?? this.failureMessage,
      origin: origin ?? this.origin,
    );
  }

  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    if (id.present) {
      map['id'] = Variable<int>(id.value);
    }
    if (type.present) {
      map['type'] = Variable<String>(type.value);
    }
    if (triggerKind.present) {
      map['trigger_kind'] = Variable<String>(triggerKind.value);
    }
    if (triggerMinutes.present) {
      map['trigger_minutes'] = Variable<int>(triggerMinutes.value);
    }
    if (triggerHour.present) {
      map['trigger_hour'] = Variable<int>(triggerHour.value);
    }
    if (triggerMinute.present) {
      map['trigger_minute'] = Variable<int>(triggerMinute.value);
    }
    if (triggerDate.present) {
      map['trigger_date'] = Variable<String>(triggerDate.value);
    }
    if (status.present) {
      map['status'] = Variable<String>(status.value);
    }
    if (targetInstantUtc.present) {
      map['target_instant_utc'] = Variable<DateTime>(targetInstantUtc.value);
    }
    if (createdAt.present) {
      map['created_at'] = Variable<DateTime>(createdAt.value);
    }
    if (updatedAt.present) {
      map['updated_at'] = Variable<DateTime>(updatedAt.value);
    }
    if (failureMessage.present) {
      map['failure_message'] = Variable<String>(failureMessage.value);
    }
    if (origin.present) {
      map['origin'] = Variable<String>(origin.value);
    }
    return map;
  }

  @override
  String toString() {
    return (StringBuffer('JobsCompanion(')
          ..write('id: $id, ')
          ..write('type: $type, ')
          ..write('triggerKind: $triggerKind, ')
          ..write('triggerMinutes: $triggerMinutes, ')
          ..write('triggerHour: $triggerHour, ')
          ..write('triggerMinute: $triggerMinute, ')
          ..write('triggerDate: $triggerDate, ')
          ..write('status: $status, ')
          ..write('targetInstantUtc: $targetInstantUtc, ')
          ..write('createdAt: $createdAt, ')
          ..write('updatedAt: $updatedAt, ')
          ..write('failureMessage: $failureMessage, ')
          ..write('origin: $origin')
          ..write(')'))
        .toString();
  }
}

class $CommandDecisionsTable extends CommandDecisions
    with TableInfo<$CommandDecisionsTable, CommandDecisionRow> {
  @override
  final GeneratedDatabase attachedDatabase;
  final String? _alias;
  $CommandDecisionsTable(this.attachedDatabase, [this._alias]);
  static const VerificationMeta _idMeta = const VerificationMeta('id');
  @override
  late final GeneratedColumn<int> id = GeneratedColumn<int>(
    'id',
    aliasedName,
    false,
    hasAutoIncrement: true,
    type: DriftSqlType.int,
    requiredDuringInsert: false,
    defaultConstraints: GeneratedColumn.constraintIsAlways(
      'PRIMARY KEY AUTOINCREMENT',
    ),
  );
  static const VerificationMeta _senderDeviceIdMeta = const VerificationMeta(
    'senderDeviceId',
  );
  @override
  late final GeneratedColumn<String> senderDeviceId = GeneratedColumn<String>(
    'sender_device_id',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _commandMeta = const VerificationMeta(
    'command',
  );
  @override
  late final GeneratedColumn<String> command = GeneratedColumn<String>(
    'command',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _decisionMeta = const VerificationMeta(
    'decision',
  );
  @override
  late final GeneratedColumn<String> decision = GeneratedColumn<String>(
    'decision',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _rejectionReasonMeta = const VerificationMeta(
    'rejectionReason',
  );
  @override
  late final GeneratedColumn<String> rejectionReason = GeneratedColumn<String>(
    'rejection_reason',
    aliasedName,
    true,
    type: DriftSqlType.string,
    requiredDuringInsert: false,
  );
  static const VerificationMeta _decidedAtMeta = const VerificationMeta(
    'decidedAt',
  );
  @override
  late final GeneratedColumn<DateTime> decidedAt = GeneratedColumn<DateTime>(
    'decided_at',
    aliasedName,
    false,
    type: DriftSqlType.dateTime,
    requiredDuringInsert: true,
  );
  @override
  List<GeneratedColumn> get $columns => [
    id,
    senderDeviceId,
    command,
    decision,
    rejectionReason,
    decidedAt,
  ];
  @override
  String get aliasedName => _alias ?? actualTableName;
  @override
  String get actualTableName => $name;
  static const String $name = 'command_decisions';
  @override
  VerificationContext validateIntegrity(
    Insertable<CommandDecisionRow> instance, {
    bool isInserting = false,
  }) {
    final context = VerificationContext();
    final data = instance.toColumns(true);
    if (data.containsKey('id')) {
      context.handle(_idMeta, id.isAcceptableOrUnknown(data['id']!, _idMeta));
    }
    if (data.containsKey('sender_device_id')) {
      context.handle(
        _senderDeviceIdMeta,
        senderDeviceId.isAcceptableOrUnknown(
          data['sender_device_id']!,
          _senderDeviceIdMeta,
        ),
      );
    } else if (isInserting) {
      context.missing(_senderDeviceIdMeta);
    }
    if (data.containsKey('command')) {
      context.handle(
        _commandMeta,
        command.isAcceptableOrUnknown(data['command']!, _commandMeta),
      );
    } else if (isInserting) {
      context.missing(_commandMeta);
    }
    if (data.containsKey('decision')) {
      context.handle(
        _decisionMeta,
        decision.isAcceptableOrUnknown(data['decision']!, _decisionMeta),
      );
    } else if (isInserting) {
      context.missing(_decisionMeta);
    }
    if (data.containsKey('rejection_reason')) {
      context.handle(
        _rejectionReasonMeta,
        rejectionReason.isAcceptableOrUnknown(
          data['rejection_reason']!,
          _rejectionReasonMeta,
        ),
      );
    }
    if (data.containsKey('decided_at')) {
      context.handle(
        _decidedAtMeta,
        decidedAt.isAcceptableOrUnknown(data['decided_at']!, _decidedAtMeta),
      );
    } else if (isInserting) {
      context.missing(_decidedAtMeta);
    }
    return context;
  }

  @override
  Set<GeneratedColumn> get $primaryKey => {id};
  @override
  CommandDecisionRow map(Map<String, dynamic> data, {String? tablePrefix}) {
    final effectivePrefix = tablePrefix != null ? '$tablePrefix.' : '';
    return CommandDecisionRow(
      id: attachedDatabase.typeMapping.read(
        DriftSqlType.int,
        data['${effectivePrefix}id'],
      )!,
      senderDeviceId: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}sender_device_id'],
      )!,
      command: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}command'],
      )!,
      decision: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}decision'],
      )!,
      rejectionReason: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}rejection_reason'],
      ),
      decidedAt: attachedDatabase.typeMapping.read(
        DriftSqlType.dateTime,
        data['${effectivePrefix}decided_at'],
      )!,
    );
  }

  @override
  $CommandDecisionsTable createAlias(String alias) {
    return $CommandDecisionsTable(attachedDatabase, alias);
  }
}

class CommandDecisionRow extends DataClass
    implements Insertable<CommandDecisionRow> {
  final int id;

  /// The device that sent the command.
  final String senderDeviceId;

  /// What it asked for, as the wire name both implementations serialise.
  final String command;

  /// `accepted` or `rejected`. Separate from the reason so a reader can count refusals
  /// without interpreting them.
  final String decision;

  /// The single refusal reason, or null when accepted.
  final String? rejectionReason;

  /// When the decision was reached.
  final DateTime decidedAt;
  const CommandDecisionRow({
    required this.id,
    required this.senderDeviceId,
    required this.command,
    required this.decision,
    this.rejectionReason,
    required this.decidedAt,
  });
  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    map['id'] = Variable<int>(id);
    map['sender_device_id'] = Variable<String>(senderDeviceId);
    map['command'] = Variable<String>(command);
    map['decision'] = Variable<String>(decision);
    if (!nullToAbsent || rejectionReason != null) {
      map['rejection_reason'] = Variable<String>(rejectionReason);
    }
    map['decided_at'] = Variable<DateTime>(decidedAt);
    return map;
  }

  CommandDecisionsCompanion toCompanion(bool nullToAbsent) {
    return CommandDecisionsCompanion(
      id: Value(id),
      senderDeviceId: Value(senderDeviceId),
      command: Value(command),
      decision: Value(decision),
      rejectionReason: rejectionReason == null && nullToAbsent
          ? const Value.absent()
          : Value(rejectionReason),
      decidedAt: Value(decidedAt),
    );
  }

  factory CommandDecisionRow.fromJson(
    Map<String, dynamic> json, {
    ValueSerializer? serializer,
  }) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return CommandDecisionRow(
      id: serializer.fromJson<int>(json['id']),
      senderDeviceId: serializer.fromJson<String>(json['senderDeviceId']),
      command: serializer.fromJson<String>(json['command']),
      decision: serializer.fromJson<String>(json['decision']),
      rejectionReason: serializer.fromJson<String?>(json['rejectionReason']),
      decidedAt: serializer.fromJson<DateTime>(json['decidedAt']),
    );
  }
  @override
  Map<String, dynamic> toJson({ValueSerializer? serializer}) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return <String, dynamic>{
      'id': serializer.toJson<int>(id),
      'senderDeviceId': serializer.toJson<String>(senderDeviceId),
      'command': serializer.toJson<String>(command),
      'decision': serializer.toJson<String>(decision),
      'rejectionReason': serializer.toJson<String?>(rejectionReason),
      'decidedAt': serializer.toJson<DateTime>(decidedAt),
    };
  }

  CommandDecisionRow copyWith({
    int? id,
    String? senderDeviceId,
    String? command,
    String? decision,
    Value<String?> rejectionReason = const Value.absent(),
    DateTime? decidedAt,
  }) => CommandDecisionRow(
    id: id ?? this.id,
    senderDeviceId: senderDeviceId ?? this.senderDeviceId,
    command: command ?? this.command,
    decision: decision ?? this.decision,
    rejectionReason: rejectionReason.present
        ? rejectionReason.value
        : this.rejectionReason,
    decidedAt: decidedAt ?? this.decidedAt,
  );
  CommandDecisionRow copyWithCompanion(CommandDecisionsCompanion data) {
    return CommandDecisionRow(
      id: data.id.present ? data.id.value : this.id,
      senderDeviceId: data.senderDeviceId.present
          ? data.senderDeviceId.value
          : this.senderDeviceId,
      command: data.command.present ? data.command.value : this.command,
      decision: data.decision.present ? data.decision.value : this.decision,
      rejectionReason: data.rejectionReason.present
          ? data.rejectionReason.value
          : this.rejectionReason,
      decidedAt: data.decidedAt.present ? data.decidedAt.value : this.decidedAt,
    );
  }

  @override
  String toString() {
    return (StringBuffer('CommandDecisionRow(')
          ..write('id: $id, ')
          ..write('senderDeviceId: $senderDeviceId, ')
          ..write('command: $command, ')
          ..write('decision: $decision, ')
          ..write('rejectionReason: $rejectionReason, ')
          ..write('decidedAt: $decidedAt')
          ..write(')'))
        .toString();
  }

  @override
  int get hashCode => Object.hash(
    id,
    senderDeviceId,
    command,
    decision,
    rejectionReason,
    decidedAt,
  );
  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is CommandDecisionRow &&
          other.id == this.id &&
          other.senderDeviceId == this.senderDeviceId &&
          other.command == this.command &&
          other.decision == this.decision &&
          other.rejectionReason == this.rejectionReason &&
          other.decidedAt == this.decidedAt);
}

class CommandDecisionsCompanion extends UpdateCompanion<CommandDecisionRow> {
  final Value<int> id;
  final Value<String> senderDeviceId;
  final Value<String> command;
  final Value<String> decision;
  final Value<String?> rejectionReason;
  final Value<DateTime> decidedAt;
  const CommandDecisionsCompanion({
    this.id = const Value.absent(),
    this.senderDeviceId = const Value.absent(),
    this.command = const Value.absent(),
    this.decision = const Value.absent(),
    this.rejectionReason = const Value.absent(),
    this.decidedAt = const Value.absent(),
  });
  CommandDecisionsCompanion.insert({
    this.id = const Value.absent(),
    required String senderDeviceId,
    required String command,
    required String decision,
    this.rejectionReason = const Value.absent(),
    required DateTime decidedAt,
  }) : senderDeviceId = Value(senderDeviceId),
       command = Value(command),
       decision = Value(decision),
       decidedAt = Value(decidedAt);
  static Insertable<CommandDecisionRow> custom({
    Expression<int>? id,
    Expression<String>? senderDeviceId,
    Expression<String>? command,
    Expression<String>? decision,
    Expression<String>? rejectionReason,
    Expression<DateTime>? decidedAt,
  }) {
    return RawValuesInsertable({
      if (id != null) 'id': id,
      if (senderDeviceId != null) 'sender_device_id': senderDeviceId,
      if (command != null) 'command': command,
      if (decision != null) 'decision': decision,
      if (rejectionReason != null) 'rejection_reason': rejectionReason,
      if (decidedAt != null) 'decided_at': decidedAt,
    });
  }

  CommandDecisionsCompanion copyWith({
    Value<int>? id,
    Value<String>? senderDeviceId,
    Value<String>? command,
    Value<String>? decision,
    Value<String?>? rejectionReason,
    Value<DateTime>? decidedAt,
  }) {
    return CommandDecisionsCompanion(
      id: id ?? this.id,
      senderDeviceId: senderDeviceId ?? this.senderDeviceId,
      command: command ?? this.command,
      decision: decision ?? this.decision,
      rejectionReason: rejectionReason ?? this.rejectionReason,
      decidedAt: decidedAt ?? this.decidedAt,
    );
  }

  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    if (id.present) {
      map['id'] = Variable<int>(id.value);
    }
    if (senderDeviceId.present) {
      map['sender_device_id'] = Variable<String>(senderDeviceId.value);
    }
    if (command.present) {
      map['command'] = Variable<String>(command.value);
    }
    if (decision.present) {
      map['decision'] = Variable<String>(decision.value);
    }
    if (rejectionReason.present) {
      map['rejection_reason'] = Variable<String>(rejectionReason.value);
    }
    if (decidedAt.present) {
      map['decided_at'] = Variable<DateTime>(decidedAt.value);
    }
    return map;
  }

  @override
  String toString() {
    return (StringBuffer('CommandDecisionsCompanion(')
          ..write('id: $id, ')
          ..write('senderDeviceId: $senderDeviceId, ')
          ..write('command: $command, ')
          ..write('decision: $decision, ')
          ..write('rejectionReason: $rejectionReason, ')
          ..write('decidedAt: $decidedAt')
          ..write(')'))
        .toString();
  }
}

class $PairingsTable extends Pairings
    with TableInfo<$PairingsTable, PairingRow> {
  @override
  final GeneratedDatabase attachedDatabase;
  final String? _alias;
  $PairingsTable(this.attachedDatabase, [this._alias]);
  static const VerificationMeta _peerDeviceIdMeta = const VerificationMeta(
    'peerDeviceId',
  );
  @override
  late final GeneratedColumn<String> peerDeviceId = GeneratedColumn<String>(
    'peer_device_id',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _verifyingKeyMeta = const VerificationMeta(
    'verifyingKey',
  );
  @override
  late final GeneratedColumn<Uint8List> verifyingKey =
      GeneratedColumn<Uint8List>(
        'verifying_key',
        aliasedName,
        false,
        type: DriftSqlType.blob,
        requiredDuringInsert: true,
      );
  static const VerificationMeta _pairedAtMeta = const VerificationMeta(
    'pairedAt',
  );
  @override
  late final GeneratedColumn<DateTime> pairedAt = GeneratedColumn<DateTime>(
    'paired_at',
    aliasedName,
    false,
    type: DriftSqlType.dateTime,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _revokedAtMeta = const VerificationMeta(
    'revokedAt',
  );
  @override
  late final GeneratedColumn<DateTime> revokedAt = GeneratedColumn<DateTime>(
    'revoked_at',
    aliasedName,
    true,
    type: DriftSqlType.dateTime,
    requiredDuringInsert: false,
  );
  @override
  List<GeneratedColumn> get $columns => [
    peerDeviceId,
    verifyingKey,
    pairedAt,
    revokedAt,
  ];
  @override
  String get aliasedName => _alias ?? actualTableName;
  @override
  String get actualTableName => $name;
  static const String $name = 'pairings';
  @override
  VerificationContext validateIntegrity(
    Insertable<PairingRow> instance, {
    bool isInserting = false,
  }) {
    final context = VerificationContext();
    final data = instance.toColumns(true);
    if (data.containsKey('peer_device_id')) {
      context.handle(
        _peerDeviceIdMeta,
        peerDeviceId.isAcceptableOrUnknown(
          data['peer_device_id']!,
          _peerDeviceIdMeta,
        ),
      );
    } else if (isInserting) {
      context.missing(_peerDeviceIdMeta);
    }
    if (data.containsKey('verifying_key')) {
      context.handle(
        _verifyingKeyMeta,
        verifyingKey.isAcceptableOrUnknown(
          data['verifying_key']!,
          _verifyingKeyMeta,
        ),
      );
    } else if (isInserting) {
      context.missing(_verifyingKeyMeta);
    }
    if (data.containsKey('paired_at')) {
      context.handle(
        _pairedAtMeta,
        pairedAt.isAcceptableOrUnknown(data['paired_at']!, _pairedAtMeta),
      );
    } else if (isInserting) {
      context.missing(_pairedAtMeta);
    }
    if (data.containsKey('revoked_at')) {
      context.handle(
        _revokedAtMeta,
        revokedAt.isAcceptableOrUnknown(data['revoked_at']!, _revokedAtMeta),
      );
    }
    return context;
  }

  @override
  Set<GeneratedColumn> get $primaryKey => {peerDeviceId};
  @override
  PairingRow map(Map<String, dynamic> data, {String? tablePrefix}) {
    final effectivePrefix = tablePrefix != null ? '$tablePrefix.' : '';
    return PairingRow(
      peerDeviceId: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}peer_device_id'],
      )!,
      verifyingKey: attachedDatabase.typeMapping.read(
        DriftSqlType.blob,
        data['${effectivePrefix}verifying_key'],
      )!,
      pairedAt: attachedDatabase.typeMapping.read(
        DriftSqlType.dateTime,
        data['${effectivePrefix}paired_at'],
      )!,
      revokedAt: attachedDatabase.typeMapping.read(
        DriftSqlType.dateTime,
        data['${effectivePrefix}revoked_at'],
      ),
    );
  }

  @override
  $PairingsTable createAlias(String alias) {
    return $PairingsTable(attachedDatabase, alias);
  }
}

class PairingRow extends DataClass implements Insertable<PairingRow> {
  /// The peer's device id, derived from its verifying key.
  final String peerDeviceId;

  /// The 32 raw bytes of the peer's Ed25519 public key.
  final Uint8List verifyingKey;
  final DateTime pairedAt;

  /// Null means the pairing is in force. Set means authority was withdrawn at that instant.
  final DateTime? revokedAt;
  const PairingRow({
    required this.peerDeviceId,
    required this.verifyingKey,
    required this.pairedAt,
    this.revokedAt,
  });
  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    map['peer_device_id'] = Variable<String>(peerDeviceId);
    map['verifying_key'] = Variable<Uint8List>(verifyingKey);
    map['paired_at'] = Variable<DateTime>(pairedAt);
    if (!nullToAbsent || revokedAt != null) {
      map['revoked_at'] = Variable<DateTime>(revokedAt);
    }
    return map;
  }

  PairingsCompanion toCompanion(bool nullToAbsent) {
    return PairingsCompanion(
      peerDeviceId: Value(peerDeviceId),
      verifyingKey: Value(verifyingKey),
      pairedAt: Value(pairedAt),
      revokedAt: revokedAt == null && nullToAbsent
          ? const Value.absent()
          : Value(revokedAt),
    );
  }

  factory PairingRow.fromJson(
    Map<String, dynamic> json, {
    ValueSerializer? serializer,
  }) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return PairingRow(
      peerDeviceId: serializer.fromJson<String>(json['peerDeviceId']),
      verifyingKey: serializer.fromJson<Uint8List>(json['verifyingKey']),
      pairedAt: serializer.fromJson<DateTime>(json['pairedAt']),
      revokedAt: serializer.fromJson<DateTime?>(json['revokedAt']),
    );
  }
  @override
  Map<String, dynamic> toJson({ValueSerializer? serializer}) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return <String, dynamic>{
      'peerDeviceId': serializer.toJson<String>(peerDeviceId),
      'verifyingKey': serializer.toJson<Uint8List>(verifyingKey),
      'pairedAt': serializer.toJson<DateTime>(pairedAt),
      'revokedAt': serializer.toJson<DateTime?>(revokedAt),
    };
  }

  PairingRow copyWith({
    String? peerDeviceId,
    Uint8List? verifyingKey,
    DateTime? pairedAt,
    Value<DateTime?> revokedAt = const Value.absent(),
  }) => PairingRow(
    peerDeviceId: peerDeviceId ?? this.peerDeviceId,
    verifyingKey: verifyingKey ?? this.verifyingKey,
    pairedAt: pairedAt ?? this.pairedAt,
    revokedAt: revokedAt.present ? revokedAt.value : this.revokedAt,
  );
  PairingRow copyWithCompanion(PairingsCompanion data) {
    return PairingRow(
      peerDeviceId: data.peerDeviceId.present
          ? data.peerDeviceId.value
          : this.peerDeviceId,
      verifyingKey: data.verifyingKey.present
          ? data.verifyingKey.value
          : this.verifyingKey,
      pairedAt: data.pairedAt.present ? data.pairedAt.value : this.pairedAt,
      revokedAt: data.revokedAt.present ? data.revokedAt.value : this.revokedAt,
    );
  }

  @override
  String toString() {
    return (StringBuffer('PairingRow(')
          ..write('peerDeviceId: $peerDeviceId, ')
          ..write('verifyingKey: $verifyingKey, ')
          ..write('pairedAt: $pairedAt, ')
          ..write('revokedAt: $revokedAt')
          ..write(')'))
        .toString();
  }

  @override
  int get hashCode => Object.hash(
    peerDeviceId,
    $driftBlobEquality.hash(verifyingKey),
    pairedAt,
    revokedAt,
  );
  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is PairingRow &&
          other.peerDeviceId == this.peerDeviceId &&
          $driftBlobEquality.equals(other.verifyingKey, this.verifyingKey) &&
          other.pairedAt == this.pairedAt &&
          other.revokedAt == this.revokedAt);
}

class PairingsCompanion extends UpdateCompanion<PairingRow> {
  final Value<String> peerDeviceId;
  final Value<Uint8List> verifyingKey;
  final Value<DateTime> pairedAt;
  final Value<DateTime?> revokedAt;
  final Value<int> rowid;
  const PairingsCompanion({
    this.peerDeviceId = const Value.absent(),
    this.verifyingKey = const Value.absent(),
    this.pairedAt = const Value.absent(),
    this.revokedAt = const Value.absent(),
    this.rowid = const Value.absent(),
  });
  PairingsCompanion.insert({
    required String peerDeviceId,
    required Uint8List verifyingKey,
    required DateTime pairedAt,
    this.revokedAt = const Value.absent(),
    this.rowid = const Value.absent(),
  }) : peerDeviceId = Value(peerDeviceId),
       verifyingKey = Value(verifyingKey),
       pairedAt = Value(pairedAt);
  static Insertable<PairingRow> custom({
    Expression<String>? peerDeviceId,
    Expression<Uint8List>? verifyingKey,
    Expression<DateTime>? pairedAt,
    Expression<DateTime>? revokedAt,
    Expression<int>? rowid,
  }) {
    return RawValuesInsertable({
      if (peerDeviceId != null) 'peer_device_id': peerDeviceId,
      if (verifyingKey != null) 'verifying_key': verifyingKey,
      if (pairedAt != null) 'paired_at': pairedAt,
      if (revokedAt != null) 'revoked_at': revokedAt,
      if (rowid != null) 'rowid': rowid,
    });
  }

  PairingsCompanion copyWith({
    Value<String>? peerDeviceId,
    Value<Uint8List>? verifyingKey,
    Value<DateTime>? pairedAt,
    Value<DateTime?>? revokedAt,
    Value<int>? rowid,
  }) {
    return PairingsCompanion(
      peerDeviceId: peerDeviceId ?? this.peerDeviceId,
      verifyingKey: verifyingKey ?? this.verifyingKey,
      pairedAt: pairedAt ?? this.pairedAt,
      revokedAt: revokedAt ?? this.revokedAt,
      rowid: rowid ?? this.rowid,
    );
  }

  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    if (peerDeviceId.present) {
      map['peer_device_id'] = Variable<String>(peerDeviceId.value);
    }
    if (verifyingKey.present) {
      map['verifying_key'] = Variable<Uint8List>(verifyingKey.value);
    }
    if (pairedAt.present) {
      map['paired_at'] = Variable<DateTime>(pairedAt.value);
    }
    if (revokedAt.present) {
      map['revoked_at'] = Variable<DateTime>(revokedAt.value);
    }
    if (rowid.present) {
      map['rowid'] = Variable<int>(rowid.value);
    }
    return map;
  }

  @override
  String toString() {
    return (StringBuffer('PairingsCompanion(')
          ..write('peerDeviceId: $peerDeviceId, ')
          ..write('verifyingKey: $verifyingKey, ')
          ..write('pairedAt: $pairedAt, ')
          ..write('revokedAt: $revokedAt, ')
          ..write('rowid: $rowid')
          ..write(')'))
        .toString();
  }
}

class $DeviceIdentitiesTable extends DeviceIdentities
    with TableInfo<$DeviceIdentitiesTable, DeviceIdentityRow> {
  @override
  final GeneratedDatabase attachedDatabase;
  final String? _alias;
  $DeviceIdentitiesTable(this.attachedDatabase, [this._alias]);
  static const VerificationMeta _idMeta = const VerificationMeta('id');
  @override
  late final GeneratedColumn<int> id = GeneratedColumn<int>(
    'id',
    aliasedName,
    false,
    type: DriftSqlType.int,
    requiredDuringInsert: false,
  );
  static const VerificationMeta _deviceIdMeta = const VerificationMeta(
    'deviceId',
  );
  @override
  late final GeneratedColumn<String> deviceId = GeneratedColumn<String>(
    'device_id',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _verifyingKeyMeta = const VerificationMeta(
    'verifyingKey',
  );
  @override
  late final GeneratedColumn<Uint8List> verifyingKey =
      GeneratedColumn<Uint8List>(
        'verifying_key',
        aliasedName,
        false,
        type: DriftSqlType.blob,
        requiredDuringInsert: true,
      );
  static const VerificationMeta _createdAtMeta = const VerificationMeta(
    'createdAt',
  );
  @override
  late final GeneratedColumn<DateTime> createdAt = GeneratedColumn<DateTime>(
    'created_at',
    aliasedName,
    false,
    type: DriftSqlType.dateTime,
    requiredDuringInsert: true,
  );
  @override
  List<GeneratedColumn> get $columns => [id, deviceId, verifyingKey, createdAt];
  @override
  String get aliasedName => _alias ?? actualTableName;
  @override
  String get actualTableName => $name;
  static const String $name = 'device_identities';
  @override
  VerificationContext validateIntegrity(
    Insertable<DeviceIdentityRow> instance, {
    bool isInserting = false,
  }) {
    final context = VerificationContext();
    final data = instance.toColumns(true);
    if (data.containsKey('id')) {
      context.handle(_idMeta, id.isAcceptableOrUnknown(data['id']!, _idMeta));
    }
    if (data.containsKey('device_id')) {
      context.handle(
        _deviceIdMeta,
        deviceId.isAcceptableOrUnknown(data['device_id']!, _deviceIdMeta),
      );
    } else if (isInserting) {
      context.missing(_deviceIdMeta);
    }
    if (data.containsKey('verifying_key')) {
      context.handle(
        _verifyingKeyMeta,
        verifyingKey.isAcceptableOrUnknown(
          data['verifying_key']!,
          _verifyingKeyMeta,
        ),
      );
    } else if (isInserting) {
      context.missing(_verifyingKeyMeta);
    }
    if (data.containsKey('created_at')) {
      context.handle(
        _createdAtMeta,
        createdAt.isAcceptableOrUnknown(data['created_at']!, _createdAtMeta),
      );
    } else if (isInserting) {
      context.missing(_createdAtMeta);
    }
    return context;
  }

  @override
  Set<GeneratedColumn> get $primaryKey => {id};
  @override
  DeviceIdentityRow map(Map<String, dynamic> data, {String? tablePrefix}) {
    final effectivePrefix = tablePrefix != null ? '$tablePrefix.' : '';
    return DeviceIdentityRow(
      id: attachedDatabase.typeMapping.read(
        DriftSqlType.int,
        data['${effectivePrefix}id'],
      )!,
      deviceId: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}device_id'],
      )!,
      verifyingKey: attachedDatabase.typeMapping.read(
        DriftSqlType.blob,
        data['${effectivePrefix}verifying_key'],
      )!,
      createdAt: attachedDatabase.typeMapping.read(
        DriftSqlType.dateTime,
        data['${effectivePrefix}created_at'],
      )!,
    );
  }

  @override
  $DeviceIdentitiesTable createAlias(String alias) {
    return $DeviceIdentitiesTable(attachedDatabase, alias);
  }
}

class DeviceIdentityRow extends DataClass
    implements Insertable<DeviceIdentityRow> {
  final int id;

  /// Derived from [verifyingKey], never assigned independently.
  final String deviceId;

  /// The 32 raw bytes of this device's Ed25519 public key. Not secret.
  final Uint8List verifyingKey;
  final DateTime createdAt;
  const DeviceIdentityRow({
    required this.id,
    required this.deviceId,
    required this.verifyingKey,
    required this.createdAt,
  });
  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    map['id'] = Variable<int>(id);
    map['device_id'] = Variable<String>(deviceId);
    map['verifying_key'] = Variable<Uint8List>(verifyingKey);
    map['created_at'] = Variable<DateTime>(createdAt);
    return map;
  }

  DeviceIdentitiesCompanion toCompanion(bool nullToAbsent) {
    return DeviceIdentitiesCompanion(
      id: Value(id),
      deviceId: Value(deviceId),
      verifyingKey: Value(verifyingKey),
      createdAt: Value(createdAt),
    );
  }

  factory DeviceIdentityRow.fromJson(
    Map<String, dynamic> json, {
    ValueSerializer? serializer,
  }) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return DeviceIdentityRow(
      id: serializer.fromJson<int>(json['id']),
      deviceId: serializer.fromJson<String>(json['deviceId']),
      verifyingKey: serializer.fromJson<Uint8List>(json['verifyingKey']),
      createdAt: serializer.fromJson<DateTime>(json['createdAt']),
    );
  }
  @override
  Map<String, dynamic> toJson({ValueSerializer? serializer}) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return <String, dynamic>{
      'id': serializer.toJson<int>(id),
      'deviceId': serializer.toJson<String>(deviceId),
      'verifyingKey': serializer.toJson<Uint8List>(verifyingKey),
      'createdAt': serializer.toJson<DateTime>(createdAt),
    };
  }

  DeviceIdentityRow copyWith({
    int? id,
    String? deviceId,
    Uint8List? verifyingKey,
    DateTime? createdAt,
  }) => DeviceIdentityRow(
    id: id ?? this.id,
    deviceId: deviceId ?? this.deviceId,
    verifyingKey: verifyingKey ?? this.verifyingKey,
    createdAt: createdAt ?? this.createdAt,
  );
  DeviceIdentityRow copyWithCompanion(DeviceIdentitiesCompanion data) {
    return DeviceIdentityRow(
      id: data.id.present ? data.id.value : this.id,
      deviceId: data.deviceId.present ? data.deviceId.value : this.deviceId,
      verifyingKey: data.verifyingKey.present
          ? data.verifyingKey.value
          : this.verifyingKey,
      createdAt: data.createdAt.present ? data.createdAt.value : this.createdAt,
    );
  }

  @override
  String toString() {
    return (StringBuffer('DeviceIdentityRow(')
          ..write('id: $id, ')
          ..write('deviceId: $deviceId, ')
          ..write('verifyingKey: $verifyingKey, ')
          ..write('createdAt: $createdAt')
          ..write(')'))
        .toString();
  }

  @override
  int get hashCode => Object.hash(
    id,
    deviceId,
    $driftBlobEquality.hash(verifyingKey),
    createdAt,
  );
  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is DeviceIdentityRow &&
          other.id == this.id &&
          other.deviceId == this.deviceId &&
          $driftBlobEquality.equals(other.verifyingKey, this.verifyingKey) &&
          other.createdAt == this.createdAt);
}

class DeviceIdentitiesCompanion extends UpdateCompanion<DeviceIdentityRow> {
  final Value<int> id;
  final Value<String> deviceId;
  final Value<Uint8List> verifyingKey;
  final Value<DateTime> createdAt;
  const DeviceIdentitiesCompanion({
    this.id = const Value.absent(),
    this.deviceId = const Value.absent(),
    this.verifyingKey = const Value.absent(),
    this.createdAt = const Value.absent(),
  });
  DeviceIdentitiesCompanion.insert({
    this.id = const Value.absent(),
    required String deviceId,
    required Uint8List verifyingKey,
    required DateTime createdAt,
  }) : deviceId = Value(deviceId),
       verifyingKey = Value(verifyingKey),
       createdAt = Value(createdAt);
  static Insertable<DeviceIdentityRow> custom({
    Expression<int>? id,
    Expression<String>? deviceId,
    Expression<Uint8List>? verifyingKey,
    Expression<DateTime>? createdAt,
  }) {
    return RawValuesInsertable({
      if (id != null) 'id': id,
      if (deviceId != null) 'device_id': deviceId,
      if (verifyingKey != null) 'verifying_key': verifyingKey,
      if (createdAt != null) 'created_at': createdAt,
    });
  }

  DeviceIdentitiesCompanion copyWith({
    Value<int>? id,
    Value<String>? deviceId,
    Value<Uint8List>? verifyingKey,
    Value<DateTime>? createdAt,
  }) {
    return DeviceIdentitiesCompanion(
      id: id ?? this.id,
      deviceId: deviceId ?? this.deviceId,
      verifyingKey: verifyingKey ?? this.verifyingKey,
      createdAt: createdAt ?? this.createdAt,
    );
  }

  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    if (id.present) {
      map['id'] = Variable<int>(id.value);
    }
    if (deviceId.present) {
      map['device_id'] = Variable<String>(deviceId.value);
    }
    if (verifyingKey.present) {
      map['verifying_key'] = Variable<Uint8List>(verifyingKey.value);
    }
    if (createdAt.present) {
      map['created_at'] = Variable<DateTime>(createdAt.value);
    }
    return map;
  }

  @override
  String toString() {
    return (StringBuffer('DeviceIdentitiesCompanion(')
          ..write('id: $id, ')
          ..write('deviceId: $deviceId, ')
          ..write('verifyingKey: $verifyingKey, ')
          ..write('createdAt: $createdAt')
          ..write(')'))
        .toString();
  }
}

abstract class _$AppDatabase extends GeneratedDatabase {
  _$AppDatabase(QueryExecutor e) : super(e);
  $AppDatabaseManager get managers => $AppDatabaseManager(this);
  late final $JobsTable jobs = $JobsTable(this);
  late final $CommandDecisionsTable commandDecisions = $CommandDecisionsTable(
    this,
  );
  late final $PairingsTable pairings = $PairingsTable(this);
  late final $DeviceIdentitiesTable deviceIdentities = $DeviceIdentitiesTable(
    this,
  );
  @override
  Iterable<TableInfo<Table, Object?>> get allTables =>
      allSchemaEntities.whereType<TableInfo<Table, Object?>>();
  @override
  List<DatabaseSchemaEntity> get allSchemaEntities => [
    jobs,
    commandDecisions,
    pairings,
    deviceIdentities,
  ];
}

typedef $$JobsTableCreateCompanionBuilder =
    JobsCompanion Function({
      Value<int> id,
      required String type,
      required String triggerKind,
      Value<int?> triggerMinutes,
      Value<int?> triggerHour,
      Value<int?> triggerMinute,
      Value<String?> triggerDate,
      required String status,
      Value<DateTime?> targetInstantUtc,
      required DateTime createdAt,
      required DateTime updatedAt,
      Value<String?> failureMessage,
      Value<String?> origin,
    });
typedef $$JobsTableUpdateCompanionBuilder =
    JobsCompanion Function({
      Value<int> id,
      Value<String> type,
      Value<String> triggerKind,
      Value<int?> triggerMinutes,
      Value<int?> triggerHour,
      Value<int?> triggerMinute,
      Value<String?> triggerDate,
      Value<String> status,
      Value<DateTime?> targetInstantUtc,
      Value<DateTime> createdAt,
      Value<DateTime> updatedAt,
      Value<String?> failureMessage,
      Value<String?> origin,
    });

class $$JobsTableFilterComposer extends Composer<_$AppDatabase, $JobsTable> {
  $$JobsTableFilterComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnFilters<int> get id => $composableBuilder(
    column: $table.id,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get type => $composableBuilder(
    column: $table.type,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get triggerKind => $composableBuilder(
    column: $table.triggerKind,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<int> get triggerMinutes => $composableBuilder(
    column: $table.triggerMinutes,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<int> get triggerHour => $composableBuilder(
    column: $table.triggerHour,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<int> get triggerMinute => $composableBuilder(
    column: $table.triggerMinute,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get triggerDate => $composableBuilder(
    column: $table.triggerDate,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get status => $composableBuilder(
    column: $table.status,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<DateTime> get targetInstantUtc => $composableBuilder(
    column: $table.targetInstantUtc,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<DateTime> get createdAt => $composableBuilder(
    column: $table.createdAt,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<DateTime> get updatedAt => $composableBuilder(
    column: $table.updatedAt,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get failureMessage => $composableBuilder(
    column: $table.failureMessage,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get origin => $composableBuilder(
    column: $table.origin,
    builder: (column) => ColumnFilters(column),
  );
}

class $$JobsTableOrderingComposer extends Composer<_$AppDatabase, $JobsTable> {
  $$JobsTableOrderingComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnOrderings<int> get id => $composableBuilder(
    column: $table.id,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get type => $composableBuilder(
    column: $table.type,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get triggerKind => $composableBuilder(
    column: $table.triggerKind,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<int> get triggerMinutes => $composableBuilder(
    column: $table.triggerMinutes,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<int> get triggerHour => $composableBuilder(
    column: $table.triggerHour,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<int> get triggerMinute => $composableBuilder(
    column: $table.triggerMinute,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get triggerDate => $composableBuilder(
    column: $table.triggerDate,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get status => $composableBuilder(
    column: $table.status,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<DateTime> get targetInstantUtc => $composableBuilder(
    column: $table.targetInstantUtc,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<DateTime> get createdAt => $composableBuilder(
    column: $table.createdAt,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<DateTime> get updatedAt => $composableBuilder(
    column: $table.updatedAt,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get failureMessage => $composableBuilder(
    column: $table.failureMessage,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get origin => $composableBuilder(
    column: $table.origin,
    builder: (column) => ColumnOrderings(column),
  );
}

class $$JobsTableAnnotationComposer
    extends Composer<_$AppDatabase, $JobsTable> {
  $$JobsTableAnnotationComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  GeneratedColumn<int> get id =>
      $composableBuilder(column: $table.id, builder: (column) => column);

  GeneratedColumn<String> get type =>
      $composableBuilder(column: $table.type, builder: (column) => column);

  GeneratedColumn<String> get triggerKind => $composableBuilder(
    column: $table.triggerKind,
    builder: (column) => column,
  );

  GeneratedColumn<int> get triggerMinutes => $composableBuilder(
    column: $table.triggerMinutes,
    builder: (column) => column,
  );

  GeneratedColumn<int> get triggerHour => $composableBuilder(
    column: $table.triggerHour,
    builder: (column) => column,
  );

  GeneratedColumn<int> get triggerMinute => $composableBuilder(
    column: $table.triggerMinute,
    builder: (column) => column,
  );

  GeneratedColumn<String> get triggerDate => $composableBuilder(
    column: $table.triggerDate,
    builder: (column) => column,
  );

  GeneratedColumn<String> get status =>
      $composableBuilder(column: $table.status, builder: (column) => column);

  GeneratedColumn<DateTime> get targetInstantUtc => $composableBuilder(
    column: $table.targetInstantUtc,
    builder: (column) => column,
  );

  GeneratedColumn<DateTime> get createdAt =>
      $composableBuilder(column: $table.createdAt, builder: (column) => column);

  GeneratedColumn<DateTime> get updatedAt =>
      $composableBuilder(column: $table.updatedAt, builder: (column) => column);

  GeneratedColumn<String> get failureMessage => $composableBuilder(
    column: $table.failureMessage,
    builder: (column) => column,
  );

  GeneratedColumn<String> get origin =>
      $composableBuilder(column: $table.origin, builder: (column) => column);
}

class $$JobsTableTableManager
    extends
        RootTableManager<
          _$AppDatabase,
          $JobsTable,
          JobRow,
          $$JobsTableFilterComposer,
          $$JobsTableOrderingComposer,
          $$JobsTableAnnotationComposer,
          $$JobsTableCreateCompanionBuilder,
          $$JobsTableUpdateCompanionBuilder,
          (JobRow, BaseReferences<_$AppDatabase, $JobsTable, JobRow>),
          JobRow,
          PrefetchHooks Function()
        > {
  $$JobsTableTableManager(_$AppDatabase db, $JobsTable table)
    : super(
        TableManagerState(
          db: db,
          table: table,
          createFilteringComposer: () =>
              $$JobsTableFilterComposer($db: db, $table: table),
          createOrderingComposer: () =>
              $$JobsTableOrderingComposer($db: db, $table: table),
          createComputedFieldComposer: () =>
              $$JobsTableAnnotationComposer($db: db, $table: table),
          updateCompanionCallback:
              ({
                Value<int> id = const Value.absent(),
                Value<String> type = const Value.absent(),
                Value<String> triggerKind = const Value.absent(),
                Value<int?> triggerMinutes = const Value.absent(),
                Value<int?> triggerHour = const Value.absent(),
                Value<int?> triggerMinute = const Value.absent(),
                Value<String?> triggerDate = const Value.absent(),
                Value<String> status = const Value.absent(),
                Value<DateTime?> targetInstantUtc = const Value.absent(),
                Value<DateTime> createdAt = const Value.absent(),
                Value<DateTime> updatedAt = const Value.absent(),
                Value<String?> failureMessage = const Value.absent(),
                Value<String?> origin = const Value.absent(),
              }) => JobsCompanion(
                id: id,
                type: type,
                triggerKind: triggerKind,
                triggerMinutes: triggerMinutes,
                triggerHour: triggerHour,
                triggerMinute: triggerMinute,
                triggerDate: triggerDate,
                status: status,
                targetInstantUtc: targetInstantUtc,
                createdAt: createdAt,
                updatedAt: updatedAt,
                failureMessage: failureMessage,
                origin: origin,
              ),
          createCompanionCallback:
              ({
                Value<int> id = const Value.absent(),
                required String type,
                required String triggerKind,
                Value<int?> triggerMinutes = const Value.absent(),
                Value<int?> triggerHour = const Value.absent(),
                Value<int?> triggerMinute = const Value.absent(),
                Value<String?> triggerDate = const Value.absent(),
                required String status,
                Value<DateTime?> targetInstantUtc = const Value.absent(),
                required DateTime createdAt,
                required DateTime updatedAt,
                Value<String?> failureMessage = const Value.absent(),
                Value<String?> origin = const Value.absent(),
              }) => JobsCompanion.insert(
                id: id,
                type: type,
                triggerKind: triggerKind,
                triggerMinutes: triggerMinutes,
                triggerHour: triggerHour,
                triggerMinute: triggerMinute,
                triggerDate: triggerDate,
                status: status,
                targetInstantUtc: targetInstantUtc,
                createdAt: createdAt,
                updatedAt: updatedAt,
                failureMessage: failureMessage,
                origin: origin,
              ),
          withReferenceMapper: (p0) => p0
              .map((e) => (e.readTable(table), BaseReferences(db, table, e)))
              .toList(),
          prefetchHooksCallback: null,
        ),
      );
}

typedef $$JobsTableProcessedTableManager =
    ProcessedTableManager<
      _$AppDatabase,
      $JobsTable,
      JobRow,
      $$JobsTableFilterComposer,
      $$JobsTableOrderingComposer,
      $$JobsTableAnnotationComposer,
      $$JobsTableCreateCompanionBuilder,
      $$JobsTableUpdateCompanionBuilder,
      (JobRow, BaseReferences<_$AppDatabase, $JobsTable, JobRow>),
      JobRow,
      PrefetchHooks Function()
    >;
typedef $$CommandDecisionsTableCreateCompanionBuilder =
    CommandDecisionsCompanion Function({
      Value<int> id,
      required String senderDeviceId,
      required String command,
      required String decision,
      Value<String?> rejectionReason,
      required DateTime decidedAt,
    });
typedef $$CommandDecisionsTableUpdateCompanionBuilder =
    CommandDecisionsCompanion Function({
      Value<int> id,
      Value<String> senderDeviceId,
      Value<String> command,
      Value<String> decision,
      Value<String?> rejectionReason,
      Value<DateTime> decidedAt,
    });

class $$CommandDecisionsTableFilterComposer
    extends Composer<_$AppDatabase, $CommandDecisionsTable> {
  $$CommandDecisionsTableFilterComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnFilters<int> get id => $composableBuilder(
    column: $table.id,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get senderDeviceId => $composableBuilder(
    column: $table.senderDeviceId,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get command => $composableBuilder(
    column: $table.command,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get decision => $composableBuilder(
    column: $table.decision,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get rejectionReason => $composableBuilder(
    column: $table.rejectionReason,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<DateTime> get decidedAt => $composableBuilder(
    column: $table.decidedAt,
    builder: (column) => ColumnFilters(column),
  );
}

class $$CommandDecisionsTableOrderingComposer
    extends Composer<_$AppDatabase, $CommandDecisionsTable> {
  $$CommandDecisionsTableOrderingComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnOrderings<int> get id => $composableBuilder(
    column: $table.id,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get senderDeviceId => $composableBuilder(
    column: $table.senderDeviceId,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get command => $composableBuilder(
    column: $table.command,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get decision => $composableBuilder(
    column: $table.decision,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get rejectionReason => $composableBuilder(
    column: $table.rejectionReason,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<DateTime> get decidedAt => $composableBuilder(
    column: $table.decidedAt,
    builder: (column) => ColumnOrderings(column),
  );
}

class $$CommandDecisionsTableAnnotationComposer
    extends Composer<_$AppDatabase, $CommandDecisionsTable> {
  $$CommandDecisionsTableAnnotationComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  GeneratedColumn<int> get id =>
      $composableBuilder(column: $table.id, builder: (column) => column);

  GeneratedColumn<String> get senderDeviceId => $composableBuilder(
    column: $table.senderDeviceId,
    builder: (column) => column,
  );

  GeneratedColumn<String> get command =>
      $composableBuilder(column: $table.command, builder: (column) => column);

  GeneratedColumn<String> get decision =>
      $composableBuilder(column: $table.decision, builder: (column) => column);

  GeneratedColumn<String> get rejectionReason => $composableBuilder(
    column: $table.rejectionReason,
    builder: (column) => column,
  );

  GeneratedColumn<DateTime> get decidedAt =>
      $composableBuilder(column: $table.decidedAt, builder: (column) => column);
}

class $$CommandDecisionsTableTableManager
    extends
        RootTableManager<
          _$AppDatabase,
          $CommandDecisionsTable,
          CommandDecisionRow,
          $$CommandDecisionsTableFilterComposer,
          $$CommandDecisionsTableOrderingComposer,
          $$CommandDecisionsTableAnnotationComposer,
          $$CommandDecisionsTableCreateCompanionBuilder,
          $$CommandDecisionsTableUpdateCompanionBuilder,
          (
            CommandDecisionRow,
            BaseReferences<
              _$AppDatabase,
              $CommandDecisionsTable,
              CommandDecisionRow
            >,
          ),
          CommandDecisionRow,
          PrefetchHooks Function()
        > {
  $$CommandDecisionsTableTableManager(
    _$AppDatabase db,
    $CommandDecisionsTable table,
  ) : super(
        TableManagerState(
          db: db,
          table: table,
          createFilteringComposer: () =>
              $$CommandDecisionsTableFilterComposer($db: db, $table: table),
          createOrderingComposer: () =>
              $$CommandDecisionsTableOrderingComposer($db: db, $table: table),
          createComputedFieldComposer: () =>
              $$CommandDecisionsTableAnnotationComposer($db: db, $table: table),
          updateCompanionCallback:
              ({
                Value<int> id = const Value.absent(),
                Value<String> senderDeviceId = const Value.absent(),
                Value<String> command = const Value.absent(),
                Value<String> decision = const Value.absent(),
                Value<String?> rejectionReason = const Value.absent(),
                Value<DateTime> decidedAt = const Value.absent(),
              }) => CommandDecisionsCompanion(
                id: id,
                senderDeviceId: senderDeviceId,
                command: command,
                decision: decision,
                rejectionReason: rejectionReason,
                decidedAt: decidedAt,
              ),
          createCompanionCallback:
              ({
                Value<int> id = const Value.absent(),
                required String senderDeviceId,
                required String command,
                required String decision,
                Value<String?> rejectionReason = const Value.absent(),
                required DateTime decidedAt,
              }) => CommandDecisionsCompanion.insert(
                id: id,
                senderDeviceId: senderDeviceId,
                command: command,
                decision: decision,
                rejectionReason: rejectionReason,
                decidedAt: decidedAt,
              ),
          withReferenceMapper: (p0) => p0
              .map((e) => (e.readTable(table), BaseReferences(db, table, e)))
              .toList(),
          prefetchHooksCallback: null,
        ),
      );
}

typedef $$CommandDecisionsTableProcessedTableManager =
    ProcessedTableManager<
      _$AppDatabase,
      $CommandDecisionsTable,
      CommandDecisionRow,
      $$CommandDecisionsTableFilterComposer,
      $$CommandDecisionsTableOrderingComposer,
      $$CommandDecisionsTableAnnotationComposer,
      $$CommandDecisionsTableCreateCompanionBuilder,
      $$CommandDecisionsTableUpdateCompanionBuilder,
      (
        CommandDecisionRow,
        BaseReferences<
          _$AppDatabase,
          $CommandDecisionsTable,
          CommandDecisionRow
        >,
      ),
      CommandDecisionRow,
      PrefetchHooks Function()
    >;
typedef $$PairingsTableCreateCompanionBuilder =
    PairingsCompanion Function({
      required String peerDeviceId,
      required Uint8List verifyingKey,
      required DateTime pairedAt,
      Value<DateTime?> revokedAt,
      Value<int> rowid,
    });
typedef $$PairingsTableUpdateCompanionBuilder =
    PairingsCompanion Function({
      Value<String> peerDeviceId,
      Value<Uint8List> verifyingKey,
      Value<DateTime> pairedAt,
      Value<DateTime?> revokedAt,
      Value<int> rowid,
    });

class $$PairingsTableFilterComposer
    extends Composer<_$AppDatabase, $PairingsTable> {
  $$PairingsTableFilterComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnFilters<String> get peerDeviceId => $composableBuilder(
    column: $table.peerDeviceId,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<Uint8List> get verifyingKey => $composableBuilder(
    column: $table.verifyingKey,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<DateTime> get pairedAt => $composableBuilder(
    column: $table.pairedAt,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<DateTime> get revokedAt => $composableBuilder(
    column: $table.revokedAt,
    builder: (column) => ColumnFilters(column),
  );
}

class $$PairingsTableOrderingComposer
    extends Composer<_$AppDatabase, $PairingsTable> {
  $$PairingsTableOrderingComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnOrderings<String> get peerDeviceId => $composableBuilder(
    column: $table.peerDeviceId,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<Uint8List> get verifyingKey => $composableBuilder(
    column: $table.verifyingKey,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<DateTime> get pairedAt => $composableBuilder(
    column: $table.pairedAt,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<DateTime> get revokedAt => $composableBuilder(
    column: $table.revokedAt,
    builder: (column) => ColumnOrderings(column),
  );
}

class $$PairingsTableAnnotationComposer
    extends Composer<_$AppDatabase, $PairingsTable> {
  $$PairingsTableAnnotationComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  GeneratedColumn<String> get peerDeviceId => $composableBuilder(
    column: $table.peerDeviceId,
    builder: (column) => column,
  );

  GeneratedColumn<Uint8List> get verifyingKey => $composableBuilder(
    column: $table.verifyingKey,
    builder: (column) => column,
  );

  GeneratedColumn<DateTime> get pairedAt =>
      $composableBuilder(column: $table.pairedAt, builder: (column) => column);

  GeneratedColumn<DateTime> get revokedAt =>
      $composableBuilder(column: $table.revokedAt, builder: (column) => column);
}

class $$PairingsTableTableManager
    extends
        RootTableManager<
          _$AppDatabase,
          $PairingsTable,
          PairingRow,
          $$PairingsTableFilterComposer,
          $$PairingsTableOrderingComposer,
          $$PairingsTableAnnotationComposer,
          $$PairingsTableCreateCompanionBuilder,
          $$PairingsTableUpdateCompanionBuilder,
          (
            PairingRow,
            BaseReferences<_$AppDatabase, $PairingsTable, PairingRow>,
          ),
          PairingRow,
          PrefetchHooks Function()
        > {
  $$PairingsTableTableManager(_$AppDatabase db, $PairingsTable table)
    : super(
        TableManagerState(
          db: db,
          table: table,
          createFilteringComposer: () =>
              $$PairingsTableFilterComposer($db: db, $table: table),
          createOrderingComposer: () =>
              $$PairingsTableOrderingComposer($db: db, $table: table),
          createComputedFieldComposer: () =>
              $$PairingsTableAnnotationComposer($db: db, $table: table),
          updateCompanionCallback:
              ({
                Value<String> peerDeviceId = const Value.absent(),
                Value<Uint8List> verifyingKey = const Value.absent(),
                Value<DateTime> pairedAt = const Value.absent(),
                Value<DateTime?> revokedAt = const Value.absent(),
                Value<int> rowid = const Value.absent(),
              }) => PairingsCompanion(
                peerDeviceId: peerDeviceId,
                verifyingKey: verifyingKey,
                pairedAt: pairedAt,
                revokedAt: revokedAt,
                rowid: rowid,
              ),
          createCompanionCallback:
              ({
                required String peerDeviceId,
                required Uint8List verifyingKey,
                required DateTime pairedAt,
                Value<DateTime?> revokedAt = const Value.absent(),
                Value<int> rowid = const Value.absent(),
              }) => PairingsCompanion.insert(
                peerDeviceId: peerDeviceId,
                verifyingKey: verifyingKey,
                pairedAt: pairedAt,
                revokedAt: revokedAt,
                rowid: rowid,
              ),
          withReferenceMapper: (p0) => p0
              .map((e) => (e.readTable(table), BaseReferences(db, table, e)))
              .toList(),
          prefetchHooksCallback: null,
        ),
      );
}

typedef $$PairingsTableProcessedTableManager =
    ProcessedTableManager<
      _$AppDatabase,
      $PairingsTable,
      PairingRow,
      $$PairingsTableFilterComposer,
      $$PairingsTableOrderingComposer,
      $$PairingsTableAnnotationComposer,
      $$PairingsTableCreateCompanionBuilder,
      $$PairingsTableUpdateCompanionBuilder,
      (PairingRow, BaseReferences<_$AppDatabase, $PairingsTable, PairingRow>),
      PairingRow,
      PrefetchHooks Function()
    >;
typedef $$DeviceIdentitiesTableCreateCompanionBuilder =
    DeviceIdentitiesCompanion Function({
      Value<int> id,
      required String deviceId,
      required Uint8List verifyingKey,
      required DateTime createdAt,
    });
typedef $$DeviceIdentitiesTableUpdateCompanionBuilder =
    DeviceIdentitiesCompanion Function({
      Value<int> id,
      Value<String> deviceId,
      Value<Uint8List> verifyingKey,
      Value<DateTime> createdAt,
    });

class $$DeviceIdentitiesTableFilterComposer
    extends Composer<_$AppDatabase, $DeviceIdentitiesTable> {
  $$DeviceIdentitiesTableFilterComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnFilters<int> get id => $composableBuilder(
    column: $table.id,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get deviceId => $composableBuilder(
    column: $table.deviceId,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<Uint8List> get verifyingKey => $composableBuilder(
    column: $table.verifyingKey,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<DateTime> get createdAt => $composableBuilder(
    column: $table.createdAt,
    builder: (column) => ColumnFilters(column),
  );
}

class $$DeviceIdentitiesTableOrderingComposer
    extends Composer<_$AppDatabase, $DeviceIdentitiesTable> {
  $$DeviceIdentitiesTableOrderingComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnOrderings<int> get id => $composableBuilder(
    column: $table.id,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get deviceId => $composableBuilder(
    column: $table.deviceId,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<Uint8List> get verifyingKey => $composableBuilder(
    column: $table.verifyingKey,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<DateTime> get createdAt => $composableBuilder(
    column: $table.createdAt,
    builder: (column) => ColumnOrderings(column),
  );
}

class $$DeviceIdentitiesTableAnnotationComposer
    extends Composer<_$AppDatabase, $DeviceIdentitiesTable> {
  $$DeviceIdentitiesTableAnnotationComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  GeneratedColumn<int> get id =>
      $composableBuilder(column: $table.id, builder: (column) => column);

  GeneratedColumn<String> get deviceId =>
      $composableBuilder(column: $table.deviceId, builder: (column) => column);

  GeneratedColumn<Uint8List> get verifyingKey => $composableBuilder(
    column: $table.verifyingKey,
    builder: (column) => column,
  );

  GeneratedColumn<DateTime> get createdAt =>
      $composableBuilder(column: $table.createdAt, builder: (column) => column);
}

class $$DeviceIdentitiesTableTableManager
    extends
        RootTableManager<
          _$AppDatabase,
          $DeviceIdentitiesTable,
          DeviceIdentityRow,
          $$DeviceIdentitiesTableFilterComposer,
          $$DeviceIdentitiesTableOrderingComposer,
          $$DeviceIdentitiesTableAnnotationComposer,
          $$DeviceIdentitiesTableCreateCompanionBuilder,
          $$DeviceIdentitiesTableUpdateCompanionBuilder,
          (
            DeviceIdentityRow,
            BaseReferences<
              _$AppDatabase,
              $DeviceIdentitiesTable,
              DeviceIdentityRow
            >,
          ),
          DeviceIdentityRow,
          PrefetchHooks Function()
        > {
  $$DeviceIdentitiesTableTableManager(
    _$AppDatabase db,
    $DeviceIdentitiesTable table,
  ) : super(
        TableManagerState(
          db: db,
          table: table,
          createFilteringComposer: () =>
              $$DeviceIdentitiesTableFilterComposer($db: db, $table: table),
          createOrderingComposer: () =>
              $$DeviceIdentitiesTableOrderingComposer($db: db, $table: table),
          createComputedFieldComposer: () =>
              $$DeviceIdentitiesTableAnnotationComposer($db: db, $table: table),
          updateCompanionCallback:
              ({
                Value<int> id = const Value.absent(),
                Value<String> deviceId = const Value.absent(),
                Value<Uint8List> verifyingKey = const Value.absent(),
                Value<DateTime> createdAt = const Value.absent(),
              }) => DeviceIdentitiesCompanion(
                id: id,
                deviceId: deviceId,
                verifyingKey: verifyingKey,
                createdAt: createdAt,
              ),
          createCompanionCallback:
              ({
                Value<int> id = const Value.absent(),
                required String deviceId,
                required Uint8List verifyingKey,
                required DateTime createdAt,
              }) => DeviceIdentitiesCompanion.insert(
                id: id,
                deviceId: deviceId,
                verifyingKey: verifyingKey,
                createdAt: createdAt,
              ),
          withReferenceMapper: (p0) => p0
              .map((e) => (e.readTable(table), BaseReferences(db, table, e)))
              .toList(),
          prefetchHooksCallback: null,
        ),
      );
}

typedef $$DeviceIdentitiesTableProcessedTableManager =
    ProcessedTableManager<
      _$AppDatabase,
      $DeviceIdentitiesTable,
      DeviceIdentityRow,
      $$DeviceIdentitiesTableFilterComposer,
      $$DeviceIdentitiesTableOrderingComposer,
      $$DeviceIdentitiesTableAnnotationComposer,
      $$DeviceIdentitiesTableCreateCompanionBuilder,
      $$DeviceIdentitiesTableUpdateCompanionBuilder,
      (
        DeviceIdentityRow,
        BaseReferences<
          _$AppDatabase,
          $DeviceIdentitiesTable,
          DeviceIdentityRow
        >,
      ),
      DeviceIdentityRow,
      PrefetchHooks Function()
    >;

class $AppDatabaseManager {
  final _$AppDatabase _db;
  $AppDatabaseManager(this._db);
  $$JobsTableTableManager get jobs => $$JobsTableTableManager(_db, _db.jobs);
  $$CommandDecisionsTableTableManager get commandDecisions =>
      $$CommandDecisionsTableTableManager(_db, _db.commandDecisions);
  $$PairingsTableTableManager get pairings =>
      $$PairingsTableTableManager(_db, _db.pairings);
  $$DeviceIdentitiesTableTableManager get deviceIdentities =>
      $$DeviceIdentitiesTableTableManager(_db, _db.deviceIdentities);
}
