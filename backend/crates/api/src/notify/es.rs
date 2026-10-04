//! Spanish versions of the messages residents, applicants and prospects get
//! (roadmap area 16). Same keys and placeholders as the English defaults; a
//! key missing here falls back to English. A workspace can override any of
//! them with a template keyed `<key>.es`.
//!
//! Values filled in by the caller (a status word, a time window, an inspection
//! kind) arrive in English for now.

use super::DefaultTemplate;

pub const ES_TEMPLATES: &[DefaultTemplate] = &[
    DefaultTemplate {
        key: "application_approved",
        subject: "Su solicitud con {company} fue aprobada",
        body: "Hola {recipient}:\n\nBuenas noticias: su solicitud de alquiler con {company} fue \
               aprobada. Pronto le escribiremos con los próximos pasos.\n\n{company}",
        sms: "{company}: buenas noticias, su solicitud de alquiler fue aprobada. Pronto le \
              enviaremos los próximos pasos.",
    },
    DefaultTemplate {
        key: "application_invite",
        subject: "Solicite con {company}",
        body: "Hola {recipient}:\n\nGracias por visitarnos. Aquí está la solicitud; toma unos \
               minutos y sus datos ya están completados:\n{apply_url}{message}\n\n{company}",
        sms: "{company}: aquí está la solicitud, con sus datos ya completados: {apply_url}",
    },
    DefaultTemplate {
        key: "application_received",
        subject: "Recibimos su solicitud",
        body: "Hola {recipient}:\n\nGracias por solicitar con {company}. Estamos revisando su \
               solicitud y le avisaremos en cuanto haya una decisión.\n\n{company}",
        sms: "{company}: gracias, recibimos su solicitud y pronto nos comunicaremos.",
    },
    DefaultTemplate {
        key: "application_declined",
        subject: "Novedades sobre su solicitud con {company}",
        body: "Hola {recipient}:\n\nGracias por solicitar con {company}. Después de revisarla con \
               cuidado, por ahora no podemos seguir adelante con su solicitud.\n\nSi tiene \
               preguntas, responda a este correo.\n\n{company}",
        sms: "{company}: lamentablemente por ahora no podemos seguir adelante con su solicitud.",
    },
    DefaultTemplate {
        key: "adverse_action",
        subject: "Aviso de acción adversa sobre su solicitud",
        body: "Hola {recipient}:\n\nEste aviso se le entrega conforme a la Ley de Informes de \
               Crédito Justos (FCRA). Su solicitud de alquiler con {company} fue rechazada, en \
               todo o en parte, por información de un informe del consumidor proporcionado por:\n\n\
               {cra_name}\n{cra_contact}\n\nLa agencia no tomó esta decisión y no puede explicar \
               por qué se tomó. Puede pedirle a la agencia una copia gratuita de su informe dentro \
               de los 60 días de este aviso, y puede disputar directamente con ella cualquier \
               información incorrecta o incompleta. El aviso completo está archivado con su \
               solicitud.\n\n{company}",
        sms: "{company}: se emitió un aviso de acción adversa sobre su solicitud. Vea su correo \
              para conocer sus derechos bajo la FCRA.",
    },
    DefaultTemplate {
        key: "appointment_offered",
        subject: "Elija una hora: {title}",
        body: "Hola {name}:\n\nNos gustaría pasar por: {title} en {property}.\n\nHorarios \
               posibles: {windows}.\n\nElija uno aquí: {link}\n\nSi ninguno le sirve, en el mismo \
               enlace puede proponer otra hora.\n\n{company}",
        sms: "{company}: elija una hora para {title}. {windows}. {link}",
    },
    DefaultTemplate {
        key: "appointment_confirmed",
        subject: "Confirmado: {title}, {when}",
        body: "Hola {name}:\n\nQuedó confirmado para {when}: {title} en {property}. Le \
               recordaremos antes.\n\n{company}",
        sms: "{company}: confirmado {when} para {title}.",
    },
    DefaultTemplate {
        key: "appointment_offer_reminder",
        subject: "Elija una hora para {title}",
        body: "Hola {name}:\n\nLe ofrecimos horarios para {title} en {property} y todavía no \
               sabemos cuál le sirve:\n{windows}\n\nElija uno aquí, o díganos una hora mejor:\n\
               {link}\n\n{company}",
        sms: "{company}: elija una hora para {title}: {link}",
    },
    DefaultTemplate {
        key: "appointment_reminder",
        subject: "Recordatorio: {title}, {when}",
        body: "Hola {name}:\n\nLe recordamos que pasaremos {in}: {when}, por {title} en \
               {property}.\n\n{company}",
        sms: "{company}: recordatorio, {title} {in}: {when}.",
    },
    DefaultTemplate {
        key: "ticket_rating_request",
        subject: "¿Cómo lo hicimos con \"{title}\"?",
        body: "Hola {name}:\n\nMarcamos \"{title}\" como terminado. ¿Salió bien? Una \
               calificación rápida nos ayuda a mejorar, y si algo no está bien, díganoslo allí y \
               volveremos.\n\n{link}\n\n{company}",
        sms: "{company}: ¿cómo lo hicimos con \"{title}\"? Califíquelo o díganos qué falta: {link}",
    },
    DefaultTemplate {
        key: "ticket_checkin",
        subject: "¿Sigue arreglado? \"{title}\"",
        body: "Hola {name}:\n\nYa pasó una semana desde que terminamos \"{title}\". ¿Todo sigue \
               funcionando? Si no, responda en la solicitud y la volveremos a abrir.\n\n{link}\n\n\
               {company}",
        sms: "{company}: después de una semana, ¿\"{title}\" sigue arreglado? Si no: {link}",
    },
    DefaultTemplate {
        key: "lead_after_showing",
        subject: "¿Listo para solicitar?",
        body: "Hola {name}:\n\nGracias por venir a ver el lugar. Si le gustó, la solicitud toma \
               unos minutos y sus datos ya están completados:\n\n{link}\n\n¿Preguntas? Solo \
               responda.\n\n{company}",
        sms: "{company}: gracias por la visita. ¿Listo para solicitar? {link}",
    },
    DefaultTemplate {
        key: "account_invite",
        subject: "{company} le creó una cuenta",
        body: "Hola {name}:\n\n{company} le creó una cuenta. Elija su contraseña aquí para \
               entrar:\n\n{link}\n\nEl enlace funciona una sola vez y vence en 7 días.\n\n{company}",
        sms: "{company} le creó una cuenta. Elija su contraseña: {link}",
    },
    DefaultTemplate {
        key: "password_reset",
        subject: "Restablezca su contraseña de {company}",
        body: "Hola {name}:\n\nAlguien pidió restablecer la contraseña de esta cuenta. Si fue \
               usted, elija una nueva aquí:\n\n{link}\n\nEl enlace funciona una sola vez y vence \
               en 24 horas. Si no fue usted, ignore este correo; su contraseña no cambió.\n\n\
               {company}",
        sms: "{company}: restablezca su contraseña aquí (vence en 24 horas): {link}",
    },
    DefaultTemplate {
        key: "esign_request",
        subject: "Se pide su firma: {document_title}",
        body: "Hola {signer}:\n\n{company} le pide que firme \"{document_title}\".\n\nRevíselo y \
               fírmelo aquí:\n{sign_url}\n\nEste enlace es solo para usted; no lo reenvíe. Al \
               firmar acepta realizar la transacción de forma electrónica (ESIGN/UETA).\n\n{company}",
        sms: "{company}: se pide su firma en {document_title}. Firmar: {sign_url}",
    },
    DefaultTemplate {
        key: "esign_reminder",
        subject: "Recordatorio: {document_title} espera su firma",
        body: "Hola {signer}:\n\nLe recordamos que \"{document_title}\" de {company} todavía \
               espera su firma.\n\nRevíselo y fírmelo aquí:\n{sign_url}\n\n{company}",
        sms: "{company}: recordatorio, {document_title} espera su firma. Firmar: {sign_url}",
    },
    DefaultTemplate {
        key: "esign_completed",
        subject: "Firmado por todos: {document_title}",
        body: "Hola {signer}:\n\nTodas las partes firmaron \"{document_title}\". La copia firmada \
               se guarda con los registros del contrato en {company}; puede pedir una copia en \
               cualquier momento.\n\n{company}",
        sms: "{company}: {document_title} está firmado por todos. La copia firmada está archivada.",
    },
    DefaultTemplate {
        key: "esign_voided",
        subject: "Pedido de firma cancelado: {document_title}",
        body: "Hola {signer}:\n\nSe canceló el pedido de firma de \"{document_title}\" de \
               {company}; no tiene que hacer nada más. Su enlace para firmar ya no funciona.\n\n\
               {company}",
        sms: "{company}: se canceló el pedido de firma de {document_title}.",
    },
    DefaultTemplate {
        key: "payment_receipt",
        subject: "Pago recibido: {amount}",
        body: "Hola {recipient}:\n\nRecibimos su pago de {amount}. Su número de recibo es \
               {receipt_number}; una copia en PDF se guarda con los registros de su contrato.\n\n\
               ¡Gracias!\n\n{company}",
        sms: "{company}: recibimos su pago de {amount}. Recibo {receipt_number}.",
    },
    DefaultTemplate {
        key: "payment_failed",
        subject: "No se pudo procesar su pago",
        body: "Hola {recipient}:\n\nNo se pudo procesar su pago de {amount}: {reason}. No se le \
               cobró nada. Intente de nuevo con otro medio de pago, o comuníquese con nosotros si \
               el problema sigue.\n\n{company}",
        sms: "{company}: su pago de {amount} no se procesó ({reason}). Intente con otro medio.",
    },
    DefaultTemplate {
        key: "autopay_failed",
        subject: "Su pago automático del alquiler no se realizó",
        body: "Hola {recipient}:\n\nNo se pudo procesar su pago automático de {amount} por el \
               alquiler con vencimiento el {due_date}: {reason}. No se le cobró nada y no \
               volveremos a intentar este pago por nuestra cuenta.\n\nPague ahora: {pay_url}\n\n\
               {company}",
        sms: "{company}: su pago automático de {amount} falló ({reason}). Pague ahora: {pay_url}",
    },
    DefaultTemplate {
        key: "rent_due",
        subject: "El alquiler de {amount} vence el {due_date}",
        body: "Hola {recipient}:\n\nLe recordamos que su alquiler de {amount} vence el \
               {due_date}.\n\nPague en línea: {pay_url}\n\nSi ya pagó, gracias, y no tenga en \
               cuenta este mensaje.\n\n{company}",
        sms: "{company}: el alquiler de {amount} vence el {due_date}. Pagar: {pay_url}",
    },
    DefaultTemplate {
        key: "rent_past_due",
        subject: "Su alquiler con vencimiento el {due_date} está sin pagar",
        body: "Hola {recipient}:\n\nNo hemos recibido su alquiler de {amount} que vencía el \
               {due_date}. Pague lo antes posible para evitar un cargo por mora.\n\nPague en \
               línea: {pay_url}\n\nSi ya pagó, gracias, y no tenga en cuenta este mensaje.\n\n\
               {company}",
        sms: "{company}: el alquiler de {amount} que vencía el {due_date} está sin pagar. \
              Pagar: {pay_url}",
    },
    DefaultTemplate {
        key: "inspection_reminder",
        subject: "Su inspección ({kind}) es {when}",
        body: "Hola {recipient}:\n\nLe recordamos que la inspección ({kind}) en {place} está \
               programada para el {date}.\n\nAgréguela a su calendario: {calendar_url}\n\nPara \
               cambiar la hora, responda a este correo o escríbanos desde su portal.\n\n{company}",
        sms: "{company}: su inspección ({kind}) en {place} es el {date}.",
    },
    DefaultTemplate {
        key: "ticket_rate_request",
        subject: "¿Cómo lo hicimos con \"{title}\"?",
        body: "Hola {recipient}:\n\nSu solicitud \"{title}\" está terminada. ¿Cómo lo hicimos? \
               Califíquela aquí: {url}\n\n{company}",
        sms: "{company}: \"{title}\" está terminado. ¿Cómo lo hicimos? Responda del 1 al 5 \
              (5 es lo mejor).",
    },
    DefaultTemplate {
        key: "ticket_rating_thanks",
        subject: "Gracias por su calificación",
        body: "Gracias por calificar \"{title}\" con {rating} de 5.\n\n{company}",
        sms: "¡Gracias! Registramos {rating}/5 para \"{title}\".",
    },
    DefaultTemplate {
        key: "repair_link",
        subject: "Envíenos una solicitud de reparación",
        body: "Parece que algo necesita arreglo. Envíe la solicitud aquí y nos encargamos: \
               {url}\n\n{company}",
        sms: "{company}: parece una reparación. Envíela a mantenimiento aquí (ya está \
              completada): {url}",
    },
    DefaultTemplate {
        key: "late_fee_applied",
        subject: "Se aplicó un cargo por mora a su cuenta",
        body: "Hola {recipient}:\n\nEl alquiler de {month} pasó su período de gracia y se aplicó \
               a su cuenta un cargo por mora de {amount}, según los términos de su contrato. Pagar \
               su saldo pendiente evita más cargos.\n\n{company}",
        sms: "{company}: se aplicó un cargo por mora de {amount} por {month}. Pague su saldo, por \
              favor.",
    },
    DefaultTemplate {
        key: "manager_message",
        subject: "Nuevo mensaje de {company}: {subject}",
        body: "Hola {recipient}:\n\nTiene un nuevo mensaje de {company} sobre su alquiler:\n\n\
               \"{preview}\"\n\nLéalo y responda desde su portal de residente, en Mensajes.\n\n\
               {company}",
        sms: "{company}: tiene un nuevo mensaje, {subject}. Responda desde su portal.",
    },
    DefaultTemplate {
        key: "maintenance_update",
        subject: "Novedades de su solicitud de mantenimiento: {title}",
        body: "Hola {recipient}:\n\nSu solicitud de mantenimiento \"{title}\" ahora está: \
               {status}.\n\nPuede seguir el avance desde su portal de residente, en Reparaciones.\
               \n\n{company}",
        sms: "{company}: su solicitud de mantenimiento \"{title}\" ahora está: {status}.",
    },
    DefaultTemplate {
        key: "maintenance_reply",
        subject: "Respuesta en su solicitud de mantenimiento: {title}",
        body: "Hola {recipient}:\n\n{author} respondió a su solicitud de mantenimiento \
               \"{title}\":\n\n\"{preview}\"\n\nLéala y responda desde su portal de residente, en \
               Reparaciones.\n\n{company}",
        sms: "{company}: {author} respondió a su solicitud \"{title}\". Véala en su portal.",
    },
    DefaultTemplate {
        key: "deposit_disposition_closed",
        subject: "Estado de cuenta de su depósito: {refund} devuelto",
        body: "Hola {recipient}:\n\nSe liquidó su depósito de garantía de {deposit}: se \
               retuvieron {withheld} ({deduction_count} deducción(es)) y se le devuelven \
               {refund}. El estado de cuenta detallado está en su portal de residente, en Mi \
               contrato.\n\n{company}",
        sms: "{company}: se liquidó su depósito, {refund} devuelto. Estado de cuenta en su portal.",
    },
];
